#![cfg(target_os = "windows")]

use std::{
    ffi::c_void,
    path::Path,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use libloading::Library;

use super::{
    BiometricError,
    secusearch::{SG400_TEMPLATE_SIZE, Sg400Template},
    storage::verify_dll_matches_architecture,
};

/// Constantes oficiales tomadas de `sgfplib.h` (FDx SDK Pro for Windows v4.3.1).
const SG_DEV_AUTO: u32 = 0xFF;
const USB_AUTO_DETECT: u32 = 0x3BD;
const TEMPLATE_FORMAT_SG400: u16 = 0x0200;
const SL_NORMAL: u32 = 5;

const SGFDX_ERROR_NONE: u32 = 0;
const SGFDX_ERROR_SYSLOAD_FAILED: u32 = 51;
const SGFDX_ERROR_INITIALIZE_FAILED: u32 = 52;
const SGFDX_ERROR_LINE_DROPPED: u32 = 53;
const SGFDX_ERROR_TIME_OUT: u32 = 54;
const SGFDX_ERROR_DEVICE_NOT_FOUND: u32 = 55;
const SGFDX_ERROR_DEV_ALREADY_OPEN: u32 = 59;

const CAPTURE_SLICE_MS: u32 = 350;
const MIN_CAPTURE_QUALITY: u32 = 50;

#[repr(C)]
#[derive(Default)]
struct SgDeviceInfoParam {
    device_id: u32,
    device_sn: [u8; 16],
    com_port: u32,
    com_speed: u32,
    image_width: u32,
    image_height: u32,
    contrast: u32,
    brightness: u32,
    gain: u32,
    image_dpi: u32,
    fw_version: u32,
}

#[repr(C)]
struct SgFingerInfo {
    finger_number: u16,
    view_number: u16,
    impression_type: u16,
    image_quality: u16,
}

type HSgfpm = *mut c_void;
type FnCreate = unsafe extern "system" fn(*mut HSgfpm) -> u32;
type FnTerminate = unsafe extern "system" fn(HSgfpm) -> u32;
type FnInit = unsafe extern "system" fn(HSgfpm, u32) -> u32;
type FnSetTemplateFormat = unsafe extern "system" fn(HSgfpm, u16) -> u32;
type FnOpenDevice = unsafe extern "system" fn(HSgfpm, u32) -> u32;
type FnCloseDevice = unsafe extern "system" fn(HSgfpm) -> u32;
type FnGetDeviceInfo = unsafe extern "system" fn(HSgfpm, *mut SgDeviceInfoParam) -> u32;
type FnGetImageEx = unsafe extern "system" fn(HSgfpm, *mut u8, u32, *mut c_void, u32) -> u32;
type FnGetImageQuality = unsafe extern "system" fn(HSgfpm, u32, u32, *const u8, *mut u32) -> u32;
type FnGetMaxTemplateSize = unsafe extern "system" fn(HSgfpm, *mut u32) -> u32;
type FnCreateTemplate =
    unsafe extern "system" fn(HSgfpm, *const SgFingerInfo, *const u8, *mut u8) -> u32;
type FnMatchTemplate =
    unsafe extern "system" fn(HSgfpm, *const u8, *const u8, u32, *mut i32) -> u32;

/// Información básica reportada por el lector físico FDx al abrir conexión.
#[derive(Debug, Clone)]
pub struct FdxDeviceMetadata {
    pub serial_number: String,
    pub image_width: u32,
    pub image_height: u32,
    pub image_dpi: u32,
    pub fw_version: u32,
}

/// Adaptador nativo de FDx SDK Pro (`sgfplib.dll`) para Windows x64.
/// Debe residir exclusivamente en el hilo dedicado al lector biométrico.
pub struct FdxReader {
    _library: Library,
    handle: HSgfpm,
    terminate: FnTerminate,
    open_device: FnOpenDevice,
    close_device: FnCloseDevice,
    get_device_info: FnGetDeviceInfo,
    get_image_ex: FnGetImageEx,
    get_image_quality: FnGetImageQuality,
    get_max_template_size: FnGetMaxTemplateSize,
    create_template: FnCreateTemplate,
    match_template: FnMatchTemplate,
    connected: bool,
    metadata: Option<FdxDeviceMetadata>,
}

impl FdxReader {
    pub fn load(dll_path: &Path) -> Result<Self, BiometricError> {
        verify_dll_matches_architecture(dll_path)?;

        // SAFETY: Firmas FFI verificadas contra `FDx SDK Pro for Windows v4.3.1/Inc/sgfplib.h` (x64, WINAPI).
        unsafe {
            let library = Library::new(dll_path).map_err(|err| {
                BiometricError::Device(format!(
                    "No se pudo cargar FDx SDK Pro ({}): {err}",
                    dll_path.display()
                ))
            })?;

            let create: FnCreate = *library
                .get(b"SGFPM_Create\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let terminate: FnTerminate = *library
                .get(b"SGFPM_Terminate\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let init: FnInit = *library
                .get(b"SGFPM_Init\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let set_template_format: FnSetTemplateFormat = *library
                .get(b"SGFPM_SetTemplateFormat\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let open_device: FnOpenDevice = *library
                .get(b"SGFPM_OpenDevice\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let close_device: FnCloseDevice = *library
                .get(b"SGFPM_CloseDevice\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let get_device_info: FnGetDeviceInfo = *library
                .get(b"SGFPM_GetDeviceInfo\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let get_image_ex: FnGetImageEx = *library
                .get(b"SGFPM_GetImageEx\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let get_image_quality: FnGetImageQuality = *library
                .get(b"SGFPM_GetImageQuality\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let get_max_template_size: FnGetMaxTemplateSize = *library
                .get(b"SGFPM_GetMaxTemplateSize\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let create_template: FnCreateTemplate = *library
                .get(b"SGFPM_CreateTemplate\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;
            let match_template: FnMatchTemplate = *library
                .get(b"SGFPM_MatchTemplate\0")
                .map_err(|err| BiometricError::Device(format!("Símbolo FDx faltante: {err}")))?;

            let mut handle: HSgfpm = ptr::null_mut();
            let create_code = create(&mut handle);
            if create_code != SGFDX_ERROR_NONE || handle.is_null() {
                return Err(BiometricError::Device(format!(
                    "SGFPM_Create falló (código {create_code})."
                )));
            }

            let init_code = init(handle, SG_DEV_AUTO);
            if init_code != SGFDX_ERROR_NONE {
                let _ = terminate(handle);
                return Err(BiometricError::Disconnected(format!(
                    "No se pudo inicializar el controlador FDx (código {init_code})."
                )));
            }

            let fmt_code = set_template_format(handle, TEMPLATE_FORMAT_SG400);
            if fmt_code != SGFDX_ERROR_NONE {
                let _ = terminate(handle);
                return Err(BiometricError::Device(format!(
                    "No se pudo configurar el formato SG400 (código {fmt_code})."
                )));
            }

            Ok(Self {
                _library: library,
                handle,
                terminate,
                open_device,
                close_device,
                get_device_info,
                get_image_ex,
                get_image_quality,
                get_max_template_size,
                create_template,
                match_template,
                connected: false,
                metadata: None,
            })
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn open(&mut self) -> Result<&FdxDeviceMetadata, BiometricError> {
        let code = unsafe { (self.open_device)(self.handle, USB_AUTO_DETECT) };
        if code != SGFDX_ERROR_NONE {
            self.connected = false;
            return Err(BiometricError::Disconnected(format!(
                "No se detectó un lector SecuGen USB disponible (código FDx {code})."
            )));
        }

        let mut info = SgDeviceInfoParam::default();
        let info_code = unsafe { (self.get_device_info)(self.handle, &mut info) };
        if info_code != SGFDX_ERROR_NONE || info.image_width == 0 || info.image_height == 0 {
            let _ = unsafe { (self.close_device)(self.handle) };
            self.connected = false;
            return Err(BiometricError::Disconnected(format!(
                "No se pudo leer parámetros del lector SecuGen (código FDx {info_code})."
            )));
        }

        let sn_end = info
            .device_sn
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(info.device_sn.len());
        let serial_number = String::from_utf8_lossy(&info.device_sn[..sn_end])
            .trim()
            .to_string();

        self.connected = true;
        self.metadata = Some(FdxDeviceMetadata {
            serial_number,
            image_width: info.image_width,
            image_height: info.image_height,
            image_dpi: info.image_dpi,
            fw_version: info.fw_version,
        });

        Ok(self.metadata.as_ref().expect("metadata was just set"))
    }

    pub fn close(&mut self) {
        if self.connected {
            let _ = unsafe { (self.close_device)(self.handle) };
            self.connected = false;
        }
    }

    /// Captura una huella de forma cancelable y devuelve una plantilla SG400 de 400 bytes.
    /// La imagen cruda se sobrescribe con ceros antes de retornar.
    pub fn capture_sg400_template(
        &mut self,
        cancel: &AtomicBool,
        total_timeout_ms: u64,
    ) -> Result<Option<Sg400Template>, BiometricError> {
        if !self.connected {
            return Err(BiometricError::Disconnected(
                "Lector SecuGen desconectado. Reconecta el equipo o usa búsqueda manual.".into(),
            ));
        }
        let Some(meta) = self.metadata.clone() else {
            return Err(BiometricError::Disconnected(
                "Sin metadatos del lector SecuGen.".into(),
            ));
        };

        let img_len = (meta.image_width as usize).saturating_mul(meta.image_height as usize);
        let mut raw_image = vec![0u8; img_len];
        let started = Instant::now();

        loop {
            if cancel.load(Ordering::SeqCst) {
                raw_image.fill(0);
                return Ok(None);
            }

            let code = unsafe {
                (self.get_image_ex)(
                    self.handle,
                    raw_image.as_mut_ptr(),
                    CAPTURE_SLICE_MS,
                    ptr::null_mut(),
                    MIN_CAPTURE_QUALITY,
                )
            };

            match code {
                SGFDX_ERROR_NONE => break,
                SGFDX_ERROR_TIME_OUT => {
                    if started.elapsed().as_millis() as u64 >= total_timeout_ms {
                        raw_image.fill(0);
                        return Err(BiometricError::Device(
                            "Tiempo de espera agotado sin detectar una huella válida.".into(),
                        ));
                    }
                    continue;
                }
                SGFDX_ERROR_DEVICE_NOT_FOUND
                | SGFDX_ERROR_LINE_DROPPED
                | SGFDX_ERROR_SYSLOAD_FAILED
                | SGFDX_ERROR_INITIALIZE_FAILED
                | SGFDX_ERROR_DEV_ALREADY_OPEN => {
                    raw_image.fill(0);
                    self.close();
                    return Err(BiometricError::Disconnected(format!(
                        "Se perdió la comunicación con el lector SecuGen (código FDx {code})."
                    )));
                }
                other => {
                    raw_image.fill(0);
                    return Err(BiometricError::Device(format!(
                        "Error al capturar imagen del lector SecuGen (código FDx {other})."
                    )));
                }
            }
        }

        if cancel.load(Ordering::SeqCst) {
            raw_image.fill(0);
            return Ok(None);
        }

        let mut quality = 0_u32;
        let _ = unsafe {
            (self.get_image_quality)(
                self.handle,
                meta.image_width,
                meta.image_height,
                raw_image.as_ptr(),
                &mut quality,
            )
        };

        let mut max_size = 0_u32;
        let max_code = unsafe { (self.get_max_template_size)(self.handle, &mut max_size) };
        if max_code != SGFDX_ERROR_NONE || (max_size as usize) < SG400_TEMPLATE_SIZE {
            raw_image.fill(0);
            return Err(BiometricError::Device(format!(
                "Tamaño máximo de plantilla SG400 inválido (código {max_code}, tamaño {max_size})."
            )));
        }

        let mut template_buf = vec![0u8; max_size as usize];
        let fp_info = SgFingerInfo {
            finger_number: 0,
            view_number: 1,
            impression_type: 0,
            image_quality: quality.min(100) as u16,
        };
        let extract_code = unsafe {
            (self.create_template)(
                self.handle,
                &fp_info,
                raw_image.as_ptr(),
                template_buf.as_mut_ptr(),
            )
        };
        raw_image.fill(0);

        if extract_code != SGFDX_ERROR_NONE {
            template_buf.fill(0);
            return Err(BiometricError::Device(format!(
                "No se pudo extraer la plantilla SG400 (código FDx {extract_code})."
            )));
        }

        let mut sg400 = [0u8; SG400_TEMPLATE_SIZE];
        sg400.copy_from_slice(&template_buf[..SG400_TEMPLATE_SIZE]);
        template_buf.fill(0);
        Ok(Some(sg400))
    }

    /// Verifica que dos capturas consecutivas durante enrolamiento correspondan al mismo dedo.
    pub fn verify_match_pair(
        &self,
        first: &Sg400Template,
        second: &Sg400Template,
    ) -> Result<bool, BiometricError> {
        let mut matched = 0_i32;
        let code = unsafe {
            (self.match_template)(
                self.handle,
                first.as_ptr(),
                second.as_ptr(),
                SL_NORMAL,
                &mut matched,
            )
        };
        if code != SGFDX_ERROR_NONE {
            return Err(BiometricError::Device(format!(
                "Error al comparar muestras de enrolamiento (código FDx {code})."
            )));
        }
        Ok(matched != 0)
    }
}

impl Drop for FdxReader {
    fn drop(&mut self) {
        self.close();
        if !self.handle.is_null() {
            let _ = unsafe { (self.terminate)(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}
