use chrono::DateTime;
use log::debug;
use serde_json::{json, Value};
use std::process::Command;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System};

fn get_windows_network_stats() -> Vec<Value> {
    let ps_output = Command::new("powershell")
        .args(["-Command", "Get-NetAdapterStatistics | Where-Object {$_.Name -notlike '*Loopback*' -and $_.Name -notlike '*Isatap*'} | Select-Object Name,ReceivedBytes,SentBytes | ConvertTo-Json"])
        .output();

    match ps_output {
        Ok(result) if result.status.success() => {
            let output_str = String::from_utf8_lossy(&result.stdout);
            if let Ok(json_data) = serde_json::from_str::<Value>(&output_str) {
                let mut network_info = Vec::new();

                // Handle both single object and array responses -
                let adapters: Vec<&Value> = if json_data.is_array() {
                    json_data.as_array().unwrap().iter().collect()
                } else {
                    vec![&json_data]
                };

                for adapter in adapters {
                    if let (Some(name), Some(received), Some(sent)) = (
                        adapter["Name"].as_str(),
                        adapter["ReceivedBytes"].as_u64(),
                        adapter["SentBytes"].as_u64(),
                    ) {
                        let received_mb = received as f64 / (1024.0 * 1024.0);
                        let sent_mb = sent as f64 / (1024.0 * 1024.0);

                        network_info.push(json!({
                            "interface": name,
                            "received_bytes": received,
                            "transmitted_bytes": sent,
                            "received_mb": format!("{:.2}", received_mb),
                            "transmitted_mb": format!("{:.2}", sent_mb),
                            "received_packets": 0,
                            "transmitted_packets": 0,
                            "errors_received": 0,
                            "errors_transmitted": 0
                        }));
                    }
                }

                if !network_info.is_empty() {
                    return network_info;
                }
            }
        }
        _ => {}
    }

    // Fallback to netstat if PowerShell fails
    let netstat_output = Command::new("netstat").args(["-e"]).output();

    match netstat_output {
        Ok(result) if result.status.success() => {
            let output_str = String::from_utf8_lossy(&result.stdout);
            let lines: Vec<&str> = output_str.lines().collect();

            // Parse netstat -e output
            for (i, line) in lines.iter().enumerate() {
                if line.contains("Bytes") && i + 1 < lines.len() {
                    let data_line = lines[i + 1];
                    let parts: Vec<&str> = data_line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let received = parts[0].parse::<u64>().unwrap_or(0);
                        let sent = parts[1].parse::<u64>().unwrap_or(0);

                        let received_mb = received as f64 / (1024.0 * 1024.0);
                        let sent_mb = sent as f64 / (1024.0 * 1024.0);

                        return vec![json!({
                            "interface": "Network Interface",
                            "received_bytes": received,
                            "transmitted_bytes": sent,
                            "received_mb": format!("{:.2}", received_mb),
                            "transmitted_mb": format!("{:.2}", sent_mb),
                            "received_packets": 0,
                            "transmitted_packets": 0,
                            "errors_received": 0,
                            "errors_transmitted": 0
                        })];
                    }
                }
            }
        }
        _ => {}
    }

    // Final fallback to sysinfo with proper refresh
    let mut networks = Networks::new();
    networks.refresh_list();
    std::thread::sleep(std::time::Duration::from_millis(100));
    networks.refresh();

    networks
        .iter()
        .map(|(interface_name, data)| {
            json!({
                "interface": interface_name,
                "received_bytes": data.received(),
                "transmitted_bytes": data.transmitted(),
                "received_mb": format!("{:.2}", data.received() as f64 / (1024.0 * 1024.0)),
                "transmitted_mb": format!("{:.2}", data.transmitted() as f64 / (1024.0 * 1024.0)),
                "received_packets": data.packets_received(),
                "transmitted_packets": data.packets_transmitted(),
                "errors_received": data.errors_on_received(),
                "errors_transmitted": data.errors_on_transmitted()
            })
        })
        .collect()
}

pub fn get_agent_details() -> Value {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    sys.refresh_all();

    // System information
    let hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
    let arch = std::env::consts::ARCH.to_string();
    let boot_time = System::boot_time();

    let boot_time_fmt = DateTime::from_timestamp(boot_time as i64, 0)
        .map(|dt| dt.format("%d/%m/%Y, %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // CPU information
    let cpu = sys
        .cpus()
        .get(0)
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    // Memory information (convert bytes to GB)
    let ram_mb = sys.total_memory() / (1024 * 1024);
    let ram_gb = ram_mb as f64 / 1024.0;

    // Disk information
    let disks = Disks::new_with_refreshed_list();
    let disk_info: Vec<Value> = disks
        .iter()
        .map(|disk| {
            let total_gb = disk.total_space() as f64 / (1024.0 * 1024.0 * 1024.0);
            let available_gb = disk.available_space() as f64 / (1024.0 * 1024.0 * 1024.0);
            let used_gb = total_gb - available_gb;
            json!({
                "name": disk.name().to_string_lossy(),
                "mount_point": disk.mount_point().to_string_lossy(),
                "total_gb": format!("{:.2}", total_gb),
                "available_gb": format!("{:.2}", available_gb),
                "used_gb": format!("{:.2}", used_gb),
                "file_system": disk.file_system().to_string_lossy(),
                "is_removable": disk.is_removable()
            })
        })
        .collect();

    // Additional system information
    let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let cpu_count = sys.cpus().len();

    // CPU usage per core
    let cpu_usage: Vec<Value> = sys
        .cpus()
        .iter()
        .enumerate()
        .map(|(i, cpu)| {
            json!({
                "core": i,
                "usage": cpu.cpu_usage(),
                "frequency": cpu.frequency()
            })
        })
        .collect();

    let network_info = get_windows_network_stats();
    // Memory details
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let available_memory = sys.available_memory();
    let total_swap = sys.total_swap();
    let used_swap = sys.used_swap();

    // Convert to MB and calculate usage percentage
    let total_mb = total_memory / (1024 * 1024);
    let used_mb = used_memory / (1024 * 1024);
    let available_mb = available_memory / (1024 * 1024);
    let usage_percent = if total_memory > 0 {
        ((used_memory as f64 / total_memory as f64) * 100.0).round() as u64
    } else {
        0
    };

    json!({
        "type": "agent_info",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "system": {
            "hostname": hostname,
            "os_name": os_name,
            "os_version": os_version,
            "kernel_version": kernel_version,
            "architecture": arch,
            "boot_time": boot_time_fmt,
            "uptime_seconds": chrono::Utc::now().timestamp() - boot_time as i64
        },
        "cpu": {
            "brand": cpu,
            "core_count": cpu_count,
            "cores": cpu_usage
        },
        "memory": {
            "total_bytes": total_memory,
            "used_bytes": used_memory,
            "available_bytes": available_memory,
            "total_mb": total_mb,
            "used_mb": used_mb,
            "available_mb": available_mb,
            "usage_percent": usage_percent,
            "total_gb": format!("{:.2}", ram_gb),
            "swap_total_bytes": total_swap,
            "swap_used_bytes": used_swap
        },
        "storage": {
            "disks": disk_info
        },
        "networking": {
            "interfaces": network_info
        }
    })
}

pub fn get_installed_software() -> Value {
    let mut system_software = Vec::new();
    let mut user_software = Vec::new();

    // For Windows, use PowerShell to get installed software
    #[cfg(windows)]
    {
        let (sys_soft, usr_soft) = get_windows_installed_software();
        system_software = sys_soft;
        user_software = usr_soft;
    }

    // For Unix-like systems, try different package managers
    #[cfg(not(windows))]
    {
        // You would need to implement similar categorization for Unix systems
        let all_software = get_unix_installed_software();
        // For now, treat all Unix software as system-wide
        system_software = all_software;
    }

    json!({
        "type": "installed_software",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "hostname": System::host_name().unwrap_or_else(|| "Unknown".to_string()),
        "system_software": system_software,
        "user_software": user_software,
        "total_system_count": system_software.len(),
        "total_user_count": user_software.len()
    })
}
#[cfg(windows)]
fn get_windows_installed_software() -> (Vec<Value>, Vec<Value>) {
    let mut system_software = Vec::new();
    let mut user_software = Vec::new();

    // Get system-wide software with better error handling
    let system_ps_command = r#"
        try {
            $software = @()
            
            # System-wide 64-bit programs
            $software += Get-ItemProperty HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\* -ErrorAction SilentlyContinue | Where-Object {$_.DisplayName -ne $null}
            
            # System-wide 32-bit programs (on 64-bit systems)
            $software += Get-ItemProperty HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\* -ErrorAction SilentlyContinue | Where-Object {$_.DisplayName -ne $null}
            
            # Remove duplicates and convert to JSON
            if ($software.Count -gt 0) {
                $software | Sort-Object DisplayName -Unique | Select-Object DisplayName, DisplayVersion, Publisher, InstallDate, EstimatedSize | ConvertTo-Json -Depth 3
            } else {
                Write-Output '[]'
            }
        } catch {
            Write-Error "Error getting system software: $($_.Exception.Message)"
            Write-Output '[]'
        }
    "#;

    // Get user-specific software with better error handling
    let user_ps_command = r#"
        try {
            $userSoftware = Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\* -ErrorAction SilentlyContinue | Where-Object {$_.DisplayName -ne $null}
            if ($userSoftware.Count -gt 0) {
                $userSoftware | Select-Object DisplayName, DisplayVersion, Publisher, InstallDate, EstimatedSize | ConvertTo-Json -Depth 3
            } else {
                Write-Output '[]'
            }
        } catch {
            Write-Error "Error getting user software: $($_.Exception.Message)"
            Write-Output '[]'
        }
    "#;

    // Process system-wide software
    match Command::new("powershell")
        .args(["-ExecutionPolicy", "Bypass", "-Command", system_ps_command])
        .output()
    {
        Ok(result) => {
            if result.status.success() {
                let output_str = String::from_utf8_lossy(&result.stdout);
                debug!("System software output: {}", output_str);

                if !output_str.trim().is_empty() && output_str.trim() != "[]" {
                    match serde_json::from_str::<Value>(&output_str) {
                        Ok(json_data) => {
                            let programs: Vec<&Value> = if json_data.is_array() {
                                json_data.as_array().unwrap().iter().collect()
                            } else {
                                vec![&json_data]
                            };

                            for program in programs {
                                if let Some(name) = program["DisplayName"].as_str() {
                                    let version =
                                        program["DisplayVersion"].as_str().unwrap_or("Unknown");
                                    let publisher =
                                        program["Publisher"].as_str().unwrap_or("Unknown");
                                    let install_date =
                                        program["InstallDate"].as_str().unwrap_or("");
                                    let size = program["EstimatedSize"].as_u64().unwrap_or(0);

                                    let size_str = if size > 0 {
                                        format!("{:.2} MB", size as f64 / 1024.0)
                                    } else {
                                        "Unknown".to_string()
                                    };

                                    system_software.push(json!({
                                        "name": name,
                                        "version": version,
                                        "publisher": publisher,
                                        "install_date": install_date,
                                        "size": size_str,
                                        "scope": "system"
                                    }));
                                }
                            }
                        }
                        Err(e) => {
                            debug!("JSON parse error for system software: {}", e);
                        }
                    }
                }
            } else {
                let error_str = String::from_utf8_lossy(&result.stderr);

                debug!("PowerShell error for system software: {}", error_str);
            }
        }
        Err(e) => {
            debug!("Failed to execute PowerShell for system software: {}", e);
        }
    }

    // Process user-specific software
    match Command::new("powershell")
        .args(["-ExecutionPolicy", "Bypass", "-Command", user_ps_command])
        .output()
    {
        Ok(result) => {
            if result.status.success() {
                let output_str = String::from_utf8_lossy(&result.stdout);

                debug!("User software output: {}", output_str); // Debug output

                if !output_str.trim().is_empty() && output_str.trim() != "[]" {
                    match serde_json::from_str::<Value>(&output_str) {
                        Ok(json_data) => {
                            let programs: Vec<&Value> = if json_data.is_array() {
                                json_data.as_array().unwrap().iter().collect()
                            } else {
                                vec![&json_data]
                            };

                            for program in programs {
                                if let Some(name) = program["DisplayName"].as_str() {
                                    let version =
                                        program["DisplayVersion"].as_str().unwrap_or("Unknown");
                                    let publisher =
                                        program["Publisher"].as_str().unwrap_or("Unknown");
                                    let install_date =
                                        program["InstallDate"].as_str().unwrap_or("");
                                    let size = program["EstimatedSize"].as_u64().unwrap_or(0);

                                    let size_str = if size > 0 {
                                        format!("{:.2} MB", size as f64 / 1024.0)
                                    } else {
                                        "Unknown".to_string()
                                    };

                                    user_software.push(json!({
                                        "name": name,
                                        "version": version,
                                        "publisher": publisher,
                                        "install_date": install_date,
                                        "size": size_str,
                                        "scope": "user"
                                    }));
                                }
                            }
                        }
                        Err(e) => {
                            debug!("JSON parse error for user software: {}", e);
                        }
                    }
                }
            } else {
                let error_str = String::from_utf8_lossy(&result.stderr);

                debug!("PowerShell error for user software: {}", error_str);
            }
        }
        Err(e) => {
            debug!("Failed to execute PowerShell for user software: {}", e);
        }
    }

    // Fallback: try WMI if both methods failed
    if system_software.is_empty() && user_software.is_empty() {
        debug!("Trying WMI fallback...");

        match Command::new("wmic")
            .args(["product", "get", "name,version,vendor", "/format:csv"])
            .output()
        {
            Ok(result) if result.status.success() => {
                let output_str = String::from_utf8_lossy(&result.stdout);
                for line in output_str.lines().skip(1) {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 4 && !parts[1].is_empty() {
                        system_software.push(json!({
                            "name": parts[1].trim(),
                            "version": parts[3].trim(),
                            "publisher": parts[2].trim(),
                            "install_date": "",
                            "size": "Unknown",
                            "scope": "system"
                        }));
                    }
                }
            }
            Ok(result) => {
                // Handle the case where command executed but failed
                let error_str = String::from_utf8_lossy(&result.stderr);

                debug!(
                    "WMI command failed with status: {}, error: {}",
                    result.status, error_str
                );
            }
            Err(e) => {
                debug!("WMI fallback failed: {}", e);
            }
        }
    }

    debug!(
        "Found {} system software, {} user software",
        system_software.len(),
        user_software.len()
    );

    (system_software, user_software)
}

#[cfg(not(windows))]
fn get_unix_installed_software() -> Vec<Value> {
    let mut software_list = Vec::new();

    // Try different package managers
    // APT (Debian/Ubuntu)
    if let Ok(output) = Command::new("dpkg-query")
        .args(["-W", "-f=${Package}\t${Version}\n"])
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    software_list.push(json!({
                        "name": parts[0],
                        "version": parts[1],
                        "publisher": "Unknown",
                        "install_date": "",
                        "size": "Unknown"
                    }));
                }
            }
            return software_list;
        }
    }

    // RPM (RedHat/CentOS/Fedora)
    if let Ok(output) = Command::new("rpm")
        .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}\n"])
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    software_list.push(json!({
                        "name": parts[0],
                        "version": parts[1],
                        "publisher": "Unknown",
                        "install_date": "",
                        "size": "Unknown"
                    }));
                }
            }
        }
    }

    software_list
}
