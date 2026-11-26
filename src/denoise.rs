//! Optional Open Image Denoise integration through its documented C API.
//! Filtering only changes display/output pixels; Monte Carlo accumulators remain untouched.
use crate::{rng::Rng, scene::Scene};
use anyhow::{Context, Result, bail, ensure};
use glam::Vec3;
use libloading::Library;
use rayon::prelude::*;
use std::{
    ffi::{CStr, c_char, c_void},
    path::PathBuf,
};

type Handle = *mut c_void;
type NewDevice = unsafe extern "C" fn(i32) -> Handle;
type Commit = unsafe extern "C" fn(Handle);
type Release = unsafe extern "C" fn(Handle);
type GetError = unsafe extern "C" fn(Handle, *mut *const c_char) -> i32;
type NewFilter = unsafe extern "C" fn(Handle, *const c_char) -> Handle;
type SetImage = unsafe extern "C" fn(
    Handle,
    *const c_char,
    *mut c_void,
    i32,
    usize,
    usize,
    usize,
    usize,
    usize,
);
type SetBool = unsafe extern "C" fn(Handle, *const c_char, bool);
type SetInt = unsafe extern "C" fn(Handle, *const c_char, i32);

struct Api {
    new_filter: NewFilter,
    release_filter: Release,
    release_device: Release,
    get_error: GetError,
    set_image: SetImage,
    set_bool: SetBool,
    set_int: SetInt,
    commit_filter: Commit,
    execute_filter: Commit,
}
/// This handle stays on its owning worker thread. It is never shared with the window thread.
pub struct Denoiser {
    api: Api,
    device: Handle,
    _library: Library,
}
struct Filter<'a> {
    api: &'a Api,
    handle: Handle,
}
impl Drop for Filter<'_> {
    fn drop(&mut self) {
        // SAFETY: This is the sole owner of a filter returned by oidnNewFilter.
        unsafe { (self.api.release_filter)(self.handle) }
    }
}
impl Drop for Denoiser {
    fn drop(&mut self) {
        // SAFETY: All borrowed filter handles have been dropped before their owning device.
        unsafe { (self.api.release_device)(self.device) }
    }
}

pub struct Guides {
    albedo: Vec<[f32; 3]>,
    normal: Vec<[f32; 3]>,
}
impl Guides {
    /// Average primary surface features with the same camera model as the beauty image.
    pub fn new(scene: &Scene, width: u32, height: u32) -> Self {
        let values: Vec<([f32; 3], [f32; 3])> = (0..width * height)
            .into_par_iter()
            .map(|index| {
                let mut albedo = Vec3::ZERO;
                let mut normal = Vec3::ZERO;
                for sample in 0..8 {
                    let mut rng = Rng::new(index, sample, 0xa07);
                    let ray =
                        scene
                            .camera
                            .ray(index % width, index / width, width, height, &mut rng);
                    let hit = scene.intersect(ray, f32::INFINITY, false);
                    let limit = hit.as_ref().map_or(f32::INFINITY, |hit| hit.t);
                    if scene
                        .medium
                        .as_ref()
                        .is_some_and(|medium| medium.sample(ray, limit, &mut rng).is_some())
                    {
                        albedo += Vec3::ONE;
                    } else if let Some(hit) = hit {
                        let material = &scene.materials[hit.material];
                        albedo += if material.is_glass() || material.is_light() {
                            Vec3::ONE
                        } else {
                            scene.surface_color(&hit)
                        };
                        normal += scene.surface_shading(&hit, -ray.direction).1;
                    } else {
                        albedo += Vec3::ONE;
                    }
                }
                (
                    (albedo / 8.0).clamp(Vec3::ZERO, Vec3::ONE).to_array(),
                    (normal / 8.0).to_array(),
                )
            })
            .collect();
        Self {
            albedo: values.iter().map(|v| v.0).collect(),
            normal: values.into_iter().map(|v| v.1).collect(),
        }
    }
}
impl Denoiser {
    pub fn new() -> Result<Self> {
        let filename = if cfg!(target_os = "macos") {
            "libOpenImageDenoise.dylib"
        } else if cfg!(target_os = "windows") {
            "OpenImageDenoise.dll"
        } else {
            "libOpenImageDenoise.so"
        };
        let path = std::env::var_os("OIDN_LIBRARY")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(".runtime/oidn/lib")
                    .join(filename)
            });
        // SAFETY: Only the documented OIDN 2.x C ABI is loaded. The Library is retained
        // for the entire lifetime of every function pointer and every OIDN object.
        unsafe {
            let library = Library::new(&path).with_context(|| {
                format!(
                    "Cannot load {}. Run ./scripts/setup-denoiser.sh",
                    path.display()
                )
            })?;
            let new_device: NewDevice = *library.get(b"oidnNewDevice\0")?;
            let commit_device: Commit = *library.get(b"oidnCommitDevice\0")?;
            let api = Api {
                new_filter: *library.get(b"oidnNewFilter\0")?,
                release_filter: *library.get(b"oidnReleaseFilter\0")?,
                release_device: *library.get(b"oidnReleaseDevice\0")?,
                get_error: *library.get(b"oidnGetDeviceError\0")?,
                set_image: *library.get(b"oidnSetSharedFilterImage\0")?,
                set_bool: *library.get(b"oidnSetFilterBool\0")?,
                set_int: *library.get(b"oidnSetFilterInt\0")?,
                commit_filter: *library.get(b"oidnCommitFilter\0")?,
                execute_filter: *library.get(b"oidnExecuteFilter\0")?,
            };
            // CPU devices support borrowed host memory on all supported platforms.
            let device = new_device(1);
            ensure!(
                !device.is_null(),
                "Open Image Denoise could not create a CPU device"
            );
            let result = Self {
                api,
                device,
                _library: library,
            };
            commit_device(device);
            result.check_error()?;
            Ok(result)
        }
    }
    fn check_error(&self) -> Result<()> {
        let mut message = std::ptr::null();
        // SAFETY: The device is valid; OIDN owns the returned, NUL-terminated error message.
        unsafe {
            if (self.api.get_error)(self.device, &mut message) != 0 {
                let detail = if message.is_null() {
                    "unknown error".into()
                } else {
                    CStr::from_ptr(message).to_string_lossy()
                };
                bail!("Open Image Denoise: {detail}");
            }
        }
        Ok(())
    }
    pub fn filter(
        &self,
        pixels: &[Vec3],
        guides: &Guides,
        width: u32,
        height: u32,
    ) -> Result<Vec<Vec3>> {
        let count = width as usize * height as usize;
        ensure!(
            width > 0
                && height > 0
                && pixels.len() == count
                && guides.albedo.len() == count
                && guides.normal.len() == count,
            "Denoising buffer dimensions do not match"
        );
        ensure!(
            pixels.iter().all(|pixel| pixel.is_finite()),
            "Denoising input contains non-finite values"
        );
        let mut color: Vec<[f32; 3]> = pixels
            .iter()
            .map(|pixel| pixel.max(Vec3::ZERO).to_array())
            .collect();
        let mut output = vec![[0.0f32; 3]; count];
        // SAFETY: Every image is count tightly packed float3s. Input arrays remain alive
        // and immutable during execution; output has exclusive mutable ownership.
        // OIDN's synchronous execute call completes before any buffer is read or dropped.
        unsafe {
            let handle = (self.api.new_filter)(self.device, c"RT".as_ptr());
            ensure!(
                !handle.is_null(),
                "Open Image Denoise could not create its RT filter"
            );
            let filter = Filter {
                api: &self.api,
                handle,
            };
            for (name, pointer) in [
                (c"color", color.as_mut_ptr().cast()),
                (c"albedo", guides.albedo.as_ptr().cast_mut().cast()),
                (c"normal", guides.normal.as_ptr().cast_mut().cast()),
                (c"output", output.as_mut_ptr().cast()),
            ] {
                (self.api.set_image)(
                    handle,
                    name.as_ptr(),
                    pointer,
                    3,
                    width as usize,
                    height as usize,
                    0,
                    0,
                    0,
                );
            }
            (self.api.set_bool)(handle, c"hdr".as_ptr(), true);
            (self.api.set_bool)(handle, c"cleanAux".as_ptr(), false);
            (self.api.set_int)(handle, c"quality".as_ptr(), 6);
            (self.api.commit_filter)(handle);
            self.check_error()?;
            (self.api.execute_filter)(handle);
            self.check_error()?;
            drop(filter);
        }
        let output: Vec<Vec3> = output.into_iter().map(Vec3::from).collect();
        ensure!(
            output.iter().all(|pixel| pixel.is_finite()),
            "Denoiser returned non-finite values"
        );
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the optional Open Image Denoise runtime"]
    fn oidn_reduces_noise_without_changing_a_constant_field() {
        let width = 64;
        let height = 64;
        let count = (width * height) as usize;
        let mut rng = Rng::new(1, 0, 42);
        let input: Vec<Vec3> = (0..count)
            .map(|_| Vec3::splat(0.4 + (rng.uniform() - 0.5) * 0.2))
            .collect();
        let guides = Guides {
            albedo: vec![[0.4; 3]; count],
            normal: vec![[0.0, 0.0, 1.0]; count],
        };
        let denoiser = Denoiser::new().unwrap();
        let output = denoiser.filter(&input, &guides, width, height).unwrap();
        let error = |pixels: &[Vec3]| {
            pixels
                .iter()
                .map(|pixel| (*pixel - Vec3::splat(0.4)).length_squared())
                .sum::<f32>()
                / count as f32
        };
        assert!(error(&output) < error(&input) * 0.3);
        assert!(output.iter().all(|pixel| pixel.is_finite()));
        assert!(
            denoiser
                .filter(&input[..1], &guides, width, height)
                .is_err()
        );
    }
}
