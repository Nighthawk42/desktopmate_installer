// main.rs
#![allow(clippy::needless_return)]
#![cfg(target_os = "windows")]

use chrono::Local;
use colored::*;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio::process::Command;
use zip::ZipArchive;
use winapi::um::wincon::SetConsoleTitleW;
use winapi::um::winnt::LPCWSTR;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Determine our base directory (where the exe is located)
    let exe_path = env::current_exe()?;
    let base_dir = exe_path.parent().unwrap_or(Path::new("."));
    let log_file = base_dir.join("DesktopMate_Install.log");
    // Ensure log file directory exists
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent)?;
    }
    write_log(&log_file, "------------------------------------------------------------")?;
    write_log(
        &log_file,
        &format!("{} - Starting DesktopMate Installer", Local::now()),
    )?;

    // Set console title.
    set_console_title("DesktopMate Installer");

    // Display symmetrical banner
    const BANNER_WIDTH: usize = 45;
    let banner_line = "=".repeat(BANNER_WIDTH);
    let title = "DesktopMate Installer";
    let padding = (BANNER_WIDTH.saturating_sub(title.len())) / 2;
    let banner_title = format!("{:padding$}{}{:padding$}", "", title, "", padding = padding);

    color_echo(ConsoleColor::Cyan, &banner_line);
    color_echo(ConsoleColor::Cyan, &banner_title);
    color_echo(ConsoleColor::Cyan, &banner_line);
    println!();

    // Prompt for installation path (default: C:\Games\DesktopMate)
    let default_path = r"C:\Games\DesktopMate";
    print!("Enter installation path (default: {}): ", default_path);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    let target_path = if input.is_empty() {
        default_path.to_owned()
    } else {
        input.to_owned()
    };
    color_echo(ConsoleColor::Green, &format!("Installation directory: {}", target_path));
    write_log(
        &log_file,
        &format!("Installation directory set to: {}", target_path),
    )?;

    // Ensure target directory exists.
    fs::create_dir_all(&target_path)?;

    // Ensure DepotDownloader.exe is available.
    let depot_downloader_dir = base_dir.join("DepotDownloader");
    let depot_downloader_exe = depot_downloader_dir.join("DepotDownloader.exe");
    let depot_downloader_zip = env::temp_dir().join("DepotDownloader.zip");
    let depot_downloader_url = "https://github.com/SteamRE/DepotDownloader/releases/latest/download/DepotDownloader-windows-x64.zip";

    if !depot_downloader_exe.exists() {
        color_echo(ConsoleColor::Yellow, "DepotDownloader.exe not found! Downloading now...");
        write_log(&log_file, "DepotDownloader not found. Initiating download.")?;

        if let Err(ex) = download_file(depot_downloader_url, &depot_downloader_zip).await {
            color_echo(
                ConsoleColor::Red,
                &format!("ERROR: Failed to download DepotDownloader! {}", ex),
            );
            write_log(&log_file, "ERROR: DepotDownloader download failed.")?;
            pause_and_exit().await;
            return Ok(());
        }

        color_echo(ConsoleColor::Green, "Extracting DepotDownloader...");
        write_log(&log_file, "Extracting DepotDownloader.")?;
        if let Err(ex) = extract_zip(&depot_downloader_zip, &depot_downloader_dir) {
            color_echo(
                ConsoleColor::Red,
                &format!("ERROR: Failed to extract DepotDownloader! {}", ex),
            );
            write_log(&log_file, "ERROR: DepotDownloader extraction failed.")?;
            pause_and_exit().await;
            return Ok(());
        }
        fs::remove_file(&depot_downloader_zip)?;

        if !depot_downloader_exe.exists() {
            color_echo(
                ConsoleColor::Red,
                "ERROR: DepotDownloader.exe still not found after extraction!",
            );
            write_log(&log_file, "ERROR: DepotDownloader.exe still missing.")?;
            pause_and_exit().await;
            return Ok(());
        } else {
            color_echo(ConsoleColor::Green, "DepotDownloader downloaded and extracted successfully.");
            write_log(&log_file, "DepotDownloader ready.")?;
        }
    }

    // STEP 1: Download the DesktopMate depot if needed.
    let desktop_mate_data_path = Path::new(&target_path).join("DesktopMate_Data");
    if !desktop_mate_data_path.exists() {
        // Prompt for Steam credentials.
        let steam_user = loop {
            print!("Enter your Steam username: ");
            io::stdout().flush()?;
            let mut user_input = String::new();
            io::stdin().read_line(&mut user_input)?;
            let trimmed = user_input.trim().to_string();
            if !trimmed.is_empty() {
                break trimmed;
            }
            println!("Steam username is required.");
        };

        let steam_pass = read_password("Enter your Steam password: ")?;
        write_log(&log_file, "Steam credentials collected.")?;

        // Build DepotDownloader arguments.
        let app_id = "3301060";
        let depot_id = "3301061";
        let manifest_id = "2467897585300615012";
        let dd_args = vec![
            "-app", app_id,
            "-depot", depot_id,
            "-manifest", manifest_id,
            "-username", &steam_user,
            "-password", &steam_pass,
            "-dir", &target_path,
        ];
        let dd_arg_string = dd_args.join(" ");
        color_echo(ConsoleColor::Blue, "Downloading DesktopMate depot (via DepotDownloader)...");
        write_log(&log_file, &format!("Running DepotDownloader with arguments: {}", dd_arg_string))?;

        let dd_exit = run_depot_downloader(&depot_downloader_exe, &dd_args).await?;
        if dd_exit != 0 {
            color_echo(
                ConsoleColor::Red,
                &format!("ERROR: DepotDownloader encountered an error. Exit code = {}", dd_exit),
            );
            write_log(&log_file, &format!("ERROR: DepotDownloader failed (exit code {}).", dd_exit))?;
            pause_and_exit().await;
            return Ok(());
        }
        color_echo(ConsoleColor::Green, "Depot download complete.");
        write_log(&log_file, "Depot download complete.")?;
    } else {
        color_echo(ConsoleColor::Yellow, "DesktopMate files already exist. Skipping depot download.");
        write_log(&log_file, "DesktopMate files already exist; skipping download.")?;
    }

    // STEP 2: Apply Goldberg Offline Patch.
    let goldberg_url = "https://gitlab.com/Mr_Goldberg/goldberg_emulator/-/jobs/4247811310/artifacts/download";
    let goldberg_zip = env::temp_dir().join(format!("goldberg_{}.zip", uuid::Uuid::new_v4()));
    let extract_path = env::temp_dir().join("goldberg_extracted");
    let patch_dll = extract_path.join("experimental").join("steam_api64.dll");
    let target_dll = Path::new(&target_path)
        .join("DesktopMate_Data")
        .join("Plugins")
        .join("x86_64")
        .join("steam_api64.dll");

    color_echo(ConsoleColor::Blue, "Downloading Goldberg patch...");
    write_log(&log_file, "Downloading Goldberg emulator patch from GitLab.")?;
    download_file(goldberg_url, &goldberg_zip).await?;

    if extract_path.exists() {
        fs::remove_dir_all(&extract_path)?;
    }
    fs::create_dir_all(&extract_path)?;
    extract_zip(&goldberg_zip, &extract_path)?;
    fs::remove_file(&goldberg_zip)?;

    if patch_dll.exists() {
        if let Some(target_dll_dir) = target_dll.parent() {
            fs::create_dir_all(target_dll_dir)?;
            fs::copy(&patch_dll, &target_dll)?;
            color_echo(ConsoleColor::Green, "Goldberg patch applied successfully.");
            write_log(&log_file, "Goldberg patch applied.")?;
        } else {
            color_echo(
                ConsoleColor::Red,
                "ERROR: Unable to determine target directory for Goldberg patch DLL.",
            );
            write_log(&log_file, "ERROR: target directory is null or empty.")?;
            pause_and_exit().await;
            return Ok(());
        }
    } else {
        color_echo(ConsoleColor::Red, "ERROR: steam_api64.dll not found in the patch archive!");
        write_log(&log_file, "ERROR: steam_api64.dll missing in goldberg archive.")?;
        pause_and_exit().await;
        return Ok(());
    }

    // STEP 3: Install MelonLoader v0.6.6 by downloading and extracting its ZIP.
    update_melonloader_if_needed(&target_path, &log_file).await?;

    // STEP 4: Install or update Custom Avatar Loader mod.
    install_or_update_custom_avatar_loader(&target_path, &log_file).await?;

    // STEP 5: Create Desktop Shortcuts.
    color_echo(ConsoleColor::Blue, "Creating desktop shortcuts...");
    write_log(&log_file, "Creating desktop shortcuts.")?;
    let desktop = match dirs::desktop_dir() {
        Some(d) => d,
        None => {
            color_echo(ConsoleColor::Red, "ERROR: Cannot determine Desktop directory.");
            pause_and_exit().await;
            return Ok(());
        }
    };
    let exe_path = Path::new(&target_path).join("DesktopMate.exe");
    let shortcut_console = desktop.join("DesktopMate_Console.lnk");
    let shortcut_no_console = desktop.join("DesktopMate_NoConsole.lnk");
    // Use PowerShell to create shortcuts.
    create_shortcut(&shortcut_console, &exe_path, &target_path, "")?;
    create_shortcut(
        &shortcut_no_console,
        &exe_path,
        &target_path,
        "melonloader.hideconsole",
    )?;
    color_echo(ConsoleColor::Green, "Desktop shortcuts created successfully.");
    write_log(&log_file, "Shortcuts created.")?;

    println!("Installation complete. Press any key to exit.");
    pause_and_exit().await;
    Ok(())
}

/// Sets the console title using the Windows API.
fn set_console_title(title: &str) {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = OsStr::new(title).encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        SetConsoleTitleW(wide.as_ptr() as LPCWSTR);
    }
}

/// Writes a colored message to the console.
enum ConsoleColor {
    Cyan,
    Green,
    Yellow,
    Blue,
    Red,
}

fn color_echo(color: ConsoleColor, message: &str) {
    match color {
        ConsoleColor::Cyan => println!("{}", message.cyan()),
        ConsoleColor::Green => println!("{}", message.green()),
        ConsoleColor::Yellow => println!("{}", message.yellow()),
        ConsoleColor::Blue => println!("{}", message.blue()),
        ConsoleColor::Red => println!("{}", message.red()),
    }
}

/// Appends a message to the log file.
fn write_log(log_file: &Path, message: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(log_file)?;
    writeln!(file, "{} - {}", Local::now(), message)?;
    Ok(())
}

/// Downloads a file from the given URL and writes it to the specified path.
async fn download_file(url: &str, output_path: &Path) -> Result<(), Box<dyn Error>> {
    let client = Client::builder().user_agent("DesktopMateInstaller").build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP error: {}", resp.status()).into());
    }
    let bytes = resp.bytes().await?;
    fs::write(output_path, &bytes)?;
    Ok(())
}

/// Extracts a zip file (at zip_path) to the specified destination directory.
fn extract_zip(zip_path: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        // Use `mangled_name()` instead of the deprecated `sanitized_name()`
        let outpath = destination.join(file.mangled_name());
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

/// Runs DepotDownloader.exe with the provided arguments and logs output.
async fn run_depot_downloader(exe_path: &Path, args: &[&str]) -> Result<i32, Box<dyn Error>> {
    let mut cmd = Command::new(exe_path);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = AsyncBufReader::new(stdout).lines();
    let mut stderr_reader = AsyncBufReader::new(stderr).lines();

    let log_file = env::current_exe()?.parent().unwrap().join("DesktopMate_Install.log");
    let stdout_log = log_file.clone();
    let stdout_handle = tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            println!("{}", line);
            let _ = write_log(&stdout_log, &format!("[DD] {}", line));
        }
    });

    let stderr_log = log_file.clone();
    let stderr_handle = tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            println!("{}", line.red());
            let _ = write_log(&stderr_log, &format!("[DD-ERR] {}", line));
        }
    });

    let status = child.wait().await?;
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;
    Ok(status.code().unwrap_or(-1))
}

/// Uses PowerShell to create a Windows shortcut.
fn create_shortcut(
    shortcut_path: &Path,
    target_path: &Path,
    working_directory: &str,
    arguments: &str,
) -> Result<(), Box<dyn Error>> {
    // Build a PowerShell command to create the shortcut via WScript.Shell.
    let script = format!(
        r#"
$WshShell = New-Object -ComObject WScript.Shell;
$Shortcut = $WshShell.CreateShortcut("{0}");
$Shortcut.TargetPath = "{1}";
$Shortcut.WorkingDirectory = "{2}";
{3}
$Shortcut.Save();
"#,
        shortcut_path.display(),
        target_path.display(),
        working_directory,
        if arguments.trim().is_empty() {
            "".to_string()
        } else {
            format!(r#"$Shortcut.Arguments = "{}";"#, arguments)
        }
    );
    // Spawn PowerShell to run the script.
    let status = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &script])
        .status()?;
    if !status.success() {
        return Err("Failed to create shortcut".into());
    }
    Ok(())
}

/// Waits for any key press and then exits.
async fn pause_and_exit() {
    println!("Press any key to exit...");
    enable_raw_mode().unwrap();
    loop {
        if event::poll(std::time::Duration::from_millis(500)).unwrap() {
            if let Event::Key(_) = event::read().unwrap() {
                break;
            }
        }
    }
    disable_raw_mode().unwrap();
    std::process::exit(0);
}

/// Reads a password from the console while masking input with asterisks.
fn read_password(prompt: &str) -> io::Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut password = String::new();
    enable_raw_mode()?;
    loop {
        if let Event::Key(key_event) = event::read()? {
            match key_event.code {
                KeyCode::Enter => {
                    println!();
                    break;
                }
                KeyCode::Backspace => {
                    if !password.is_empty() {
                        password.pop();
                        print!("\r{} \r", "*".repeat(password.len()));
                        io::stdout().flush()?;
                    }
                }
                KeyCode::Char(c) => {
                    password.push(c);
                    print!("*");
                    io::stdout().flush()?;
                }
                _ => {}
            }
        }
    }
    disable_raw_mode()?;
    Ok(password)
}

/// Structure to store GitHub release info.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Helper structure for release info.
struct ReleaseInfo {
    tag_name: String,
    download_url: String,
}

/// Retrieves the latest release info from GitHub.
async fn get_latest_release(
    owner: &str,
    repo: &str,
    asset_name_filter: Option<&str>,
) -> Option<ReleaseInfo> {
    let url = format!("https://api.github.com/repos/{}/{}/releases/latest", owner, repo);
    let client = Client::builder().user_agent("DesktopMateInstaller").build().ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let release: GitHubRelease = resp.json().await.ok()?;
    let mut download_url = String::new();
    for asset in release.assets {
        if let Some(filter) = asset_name_filter {
            if asset.name.eq_ignore_ascii_case(filter) {
                download_url = asset.browser_download_url;
                break;
            }
        } else if asset.name.to_lowercase().ends_with(".zip") {
            download_url = asset.browser_download_url;
            break;
        }
    }
    // Fallback for MelonLoader.
    if download_url.is_empty() && repo.eq_ignore_ascii_case("MelonLoader") {
        download_url = "https://github.com/LavaGang/MelonLoader/releases/latest/download/MelonLoader.x64.zip".to_owned();
    }
    Some(ReleaseInfo { tag_name: release.tag_name, download_url })
}

/// Installs MelonLoader version 0.6.6 by downloading and extracting its ZIP into the game directory.
async fn update_melonloader_if_needed(target_path: &str, log_file: &Path) -> Result<(), Box<dyn Error>> {
    let version_file = Path::new(target_path).join("MelonLoader.version");
    let installed_version = if version_file.exists() {
        fs::read_to_string(&version_file)?.trim().to_string()
    } else {
        String::new()
    };

    let desired_version = "v0.6.6";
    if installed_version == desired_version {
        color_echo(ConsoleColor::Green, &format!("MelonLoader is up-to-date (version {}).", installed_version));
        write_log(log_file, &format!("MelonLoader up-to-date (version {}).", installed_version))?;
        return Ok(());
    }

    color_echo(ConsoleColor::Yellow, &format!("Installing MelonLoader {}...", desired_version));
    write_log(log_file, &format!("Downloading MelonLoader {} zip.", desired_version))?;

    let melon_zip_url = "https://github.com/LavaGang/MelonLoader/releases/download/v0.6.6/MelonLoader.x64.zip";
    let melon_zip_path = env::temp_dir().join("MelonLoader.x64.zip");
    download_file(melon_zip_url, &melon_zip_path).await?;

    color_echo(ConsoleColor::Blue, "Extracting MelonLoader contents to game directory...");
    write_log(log_file, "Extracting MelonLoader contents to game directory.")?;
    extract_zip(&melon_zip_path, Path::new(target_path))?;
    fs::remove_file(&melon_zip_path)?;
    fs::write(&version_file, desired_version)?;
    color_echo(ConsoleColor::Green, "MelonLoader installed successfully.");
    write_log(log_file, "MelonLoader installed successfully.")?;
    Ok(())
}

/// Installs or updates the Custom Avatar Loader mod.
/// It now checks for both the "Mods" and "UserLibs" folders and copies them into the game directory.
async fn install_or_update_custom_avatar_loader(target_path: &str, log_file: &Path) -> Result<(), Box<dyn Error>> {
    let version_file = Path::new(target_path).join("CustomAvatarLoader.version");
    let installed_version = if version_file.exists() {
        fs::read_to_string(&version_file)?.trim().to_string()
    } else {
        String::new()
    };

    color_echo(ConsoleColor::Blue, "Checking for Custom Avatar Loader mod updates...");
    write_log(log_file, "Checking for Custom Avatar Loader mod updates.")?;
    if let Some(latest_release) = get_latest_release("YusufOzmen01", "desktopmate-custom-avatar-loader", Some("CustomAvatarLoader.zip")).await {
        if installed_version == latest_release.tag_name {
            color_echo(ConsoleColor::Green, &format!("Custom Avatar Loader mod is up-to-date (version {}).", installed_version));
            write_log(log_file, &format!("Custom Avatar Loader mod up-to-date (version {}).", installed_version))?;
        } else {
            if installed_version.is_empty() {
                color_echo(ConsoleColor::Yellow, "Custom Avatar Loader mod not installed. Installing now...");
                write_log(log_file, "Custom Avatar Loader mod not installed. Installing.")?;
            } else {
                color_echo(ConsoleColor::Yellow, &format!(
                    "Custom Avatar Loader mod update available: Installed version: {}, Latest version: {}",
                    installed_version, latest_release.tag_name
                ));
                write_log(log_file, &format!(
                    "Custom Avatar Loader mod update available: Installed version: {}, Latest version: {}",
                    installed_version, latest_release.tag_name
                ))?;
                print!("Do you want to update Custom Avatar Loader mod? (Y/N): ");
                io::stdout().flush()?;
                let mut response = String::new();
                io::stdin().read_line(&mut response)?;
                if response.trim().to_uppercase() != "Y" {
                    color_echo(ConsoleColor::Yellow, "Skipping Custom Avatar Loader mod update.");
                    write_log(log_file, "User opted to skip Custom Avatar Loader mod update.")?;
                    return Ok(());
                }
            }
            let mod_zip = env::temp_dir().join(format!("custom_avatar_{}.zip", uuid::Uuid::new_v4()));
            color_echo(ConsoleColor::Blue, "Downloading Custom Avatar Loader mod...");
            write_log(log_file, &format!("Downloading Custom Avatar Loader mod from {}", latest_release.download_url))?;
            download_file(&latest_release.download_url, &mod_zip).await.map_err(|e| {
                color_echo(ConsoleColor::Red, &format!("ERROR: Failed to download Custom Avatar Loader mod: {}", e));
                write_log(log_file, "ERROR: Custom Avatar Loader mod download failed.").unwrap();
                e
            })?;
            let extract_path = env::temp_dir().join("custom_avatar_loader_extracted");
            if extract_path.exists() {
                fs::remove_dir_all(&extract_path)?;
            }
            fs::create_dir_all(&extract_path)?;
            extract_zip(&mod_zip, &extract_path)?;
            fs::remove_file(&mod_zip)?;

            // If the ZIP contains a single folder, use it as the root.
            let root_extracted = {
                let dirs: Vec<_> = fs::read_dir(&extract_path)?
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_dir())
                    .collect();
                if dirs.len() == 1 {
                    dirs[0].path()
                } else {
                    extract_path.clone()
                }
            };

            let mut copied_something = false;
            let mods_source = root_extracted.join("Mods");
            if mods_source.exists() {
                copy_directory(&mods_source, &Path::new(target_path).join("Mods"))?;
                copied_something = true;
            }
            let userlibs_source = root_extracted.join("UserLibs");
            if userlibs_source.exists() {
                copy_directory(&userlibs_source, &Path::new(target_path).join("UserLibs"))?;
                copied_something = true;
            }
            fs::remove_dir_all(&extract_path)?;
            if !copied_something {
                color_echo(ConsoleColor::Red, "ERROR: Neither 'Mods' nor 'UserLibs' directory found in the extracted archive!");
                write_log(log_file, "ERROR: Extracted mod archive does not contain expected 'Mods' or 'UserLibs' directories.")?;
                pause_and_exit().await;
            }
            fs::write(&version_file, &latest_release.tag_name)?;
            color_echo(ConsoleColor::Green, "Custom Avatar Loader mod installed/updated successfully.");
            write_log(log_file, "Custom Avatar Loader mod installed/updated.")?;
        }
    } else {
        color_echo(ConsoleColor::Yellow, "Could not retrieve latest Custom Avatar Loader mod release info. Skipping update check.");
        write_log(log_file, "Failed to get latest Custom Avatar Loader mod release info.")?;
    }
    Ok(())
}

/// Recursively copies a directory from source to destination.
fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = destination.join(entry.file_name());
        if path.is_dir() {
            copy_directory(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}
