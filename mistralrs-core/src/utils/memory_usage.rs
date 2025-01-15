use std::process::Command;

use candle_core::{Device, Result};
use sysinfo::System;

fn get_available_memory_vm_stat() -> Result<usize> {
    // Execute the `vm_stat` command
    let output = Command::new("vm_stat")
        .output()
        .map_err(|e| candle_core::Error::msg(format!("Failed to execute vm_stat: {}", e)))?;

    // Convert output to a string
    let output_str = String::from_utf8(output.stdout)
        .map_err(|e| candle_core::Error::msg(format!("Failed to parse output: {}", e)))?;

    // Initialize variables
    let mut free_pages = 0;
    let mut inactive_pages = 0;
    let mut page_size = 0; // Default page size in bytes for macOS

    // Parse the output line by line
    for line in output_str.lines() {
        if line.starts_with("Pages free:") {
            if let Some(value) = line.split_whitespace().nth(2) {
                free_pages = value.trim_end_matches('.').parse::<usize>().unwrap();
            }
        } else if line.starts_with("Pages inactive:") {
            if let Some(value) = line.split_whitespace().nth(2) {
                inactive_pages = value.trim_end_matches('.').parse::<usize>().unwrap();
            }
        } else if line.starts_with("Mach Virtual Memory Statistics:") {
            if let Some(start) = line.find("of ") {
                if let Some(end) = line.find(" bytes)") {
                    page_size = (line[start + "of ".len()..end].to_string())
                        .parse::<usize>()
                        .unwrap();
                }
            }
        }
    }

    // Calculate available memory
    let available_memory = (free_pages + inactive_pages) * page_size;

    Ok(available_memory)
}

fn get_total_memory_vm_stat() -> Result<usize> {
    // Execute the `vm_stat` command
    let output = Command::new("sysctl")
        .arg("hw.memsize")
        .output()
        .map_err(|e| candle_core::Error::msg(format!("Failed to execute sysctl: {}", e)))?;

    // Convert output to a string
    let output_str = String::from_utf8(output.stdout)
        .map_err(|e| candle_core::Error::msg(format!("Failed to parse output: {}", e)))?;

    Ok(output_str
        .trim_start_matches("hw.memsize: ")
        .trim()
        .parse::<usize>()
        .unwrap())
}

pub struct MemoryUsage;

impl MemoryUsage {
    /// Amount of available memory in bytes.
    pub fn get_memory_available(&self, device: &Device) -> Result<usize> {
        match device {
            Device::Cpu => {
                let mut sys = System::new_all();
                sys.refresh_cpu();
                Ok(usize::try_from(sys.available_memory())?)
            }
            #[cfg(feature = "cuda")]
            Device::Cuda(dev) => {
                use candle_core::cuda::cudarc;
                use candle_core::cuda_backend::WrapErr;
                use candle_core::{backend::BackendDevice, DeviceLocation};

                let DeviceLocation::Cuda { gpu_id } = dev.location() else {
                    candle_core::bail!("device and location do match")
                };

                let original_ctx = dev.cu_primary_ctx();

                let avail_mem = {
                    let cu_device = cudarc::driver::result::device::get(gpu_id as i32).w()?;

                    // primary context initialization, can fail with OOM
                    let cu_primary_ctx =
                        unsafe { cudarc::driver::result::primary_ctx::retain(cu_device) }.w()?;

                    unsafe { cudarc::driver::result::ctx::set_current(cu_primary_ctx) }.unwrap();

                    let res = cudarc::driver::result::mem_get_info().w()?.0;

                    unsafe { cudarc::driver::result::primary_ctx::release(cu_device) }.unwrap();

                    res
                };

                unsafe { cudarc::driver::result::ctx::set_current(*original_ctx) }.unwrap();

                Ok(avail_mem)
            }
            #[cfg(not(feature = "cuda"))]
            Device::Cuda(_) => {
                candle_core::bail!("Cannot get memory available for CUDA device")
            }
            Device::Metal(_) => get_available_memory_vm_stat(),
        }
    }

    /// Amount of total memory in bytes.
    pub fn get_total_memory(&self, device: &Device) -> Result<usize> {
        match device {
            Device::Cpu => {
                let mut sys = System::new_all();
                sys.refresh_cpu();
                Ok(usize::try_from(sys.total_memory())?)
            }
            #[cfg(feature = "cuda")]
            Device::Cuda(dev) => {
                use candle_core::cuda::cudarc;
                use candle_core::cuda_backend::WrapErr;
                use candle_core::{backend::BackendDevice, DeviceLocation};

                let DeviceLocation::Cuda { gpu_id } = dev.location() else {
                    candle_core::bail!("device and location do match")
                };

                let original_ctx = dev.cu_primary_ctx();

                let total_mem = {
                    let cu_device = cudarc::driver::result::device::get(gpu_id as i32).w()?;

                    // primary context initialization, can fail with OOM
                    let cu_primary_ctx =
                        unsafe { cudarc::driver::result::primary_ctx::retain(cu_device) }.w()?;

                    unsafe { cudarc::driver::result::ctx::set_current(cu_primary_ctx) }.unwrap();

                    let res = cudarc::driver::result::mem_get_info().w()?.1;

                    unsafe { cudarc::driver::result::primary_ctx::release(cu_device) }.unwrap();

                    res
                };

                unsafe { cudarc::driver::result::ctx::set_current(*original_ctx) }.unwrap();

                Ok(total_mem)
            }
            #[cfg(not(feature = "cuda"))]
            Device::Cuda(_) => {
                candle_core::bail!("Cannot get total memory for CUDA device")
            }
            Device::Metal(_) => get_total_memory_vm_stat(),
        }
    }
}
