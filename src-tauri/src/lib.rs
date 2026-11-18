use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

const WINDOWS_REPO: &str = "kardswalker/KMod-Loader-Windows";
const ANDROID_REPO: &str = "CCB-TEAM/KMod-Loader-Android";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderRelease {
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub published_at: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: String,
    prerelease: bool,
    published_at: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub platform: String,
    pub flavor: String,
    pub version: String,
    pub asset_name: String,
    pub path: String,
    pub installed: bool,
    pub detail: String,
    pub sha256: String,
}

fn repository(platform: &str) -> Result<&'static str, String> {
    match platform {
        "windows" => Ok(WINDOWS_REPO),
        "android" => Ok(ANDROID_REPO),
        _ => Err(format!("unsupported platform: {platform}")),
    }
}

fn fetch_releases(platform: &str) -> Result<Vec<LoaderRelease>, String> {
    let repo = repository(platform)?;
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=30");
    let response = ureq::get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "kmod-launcher/0.1")
        .call()
        .map_err(|e| format!("GitHub release request failed: {e}"))?;
    let releases: Vec<GithubRelease> = response
        .into_json()
        .map_err(|e| format!("invalid GitHub release response: {e}"))?;
    Ok(releases
        .into_iter()
        .map(|release| LoaderRelease {
            tag_name: release.tag_name,
            name: release.name,
            prerelease: release.prerelease,
            published_at: release.published_at,
            assets: release
                .assets
                .into_iter()
                .map(|asset| ReleaseAsset {
                    name: asset.name,
                    download_url: asset.browser_download_url,
                    size: asset.size,
                })
                .collect(),
        })
        .collect())
}

#[tauri::command]
fn list_releases(platform: String) -> Result<Vec<LoaderRelease>, String> {
    fetch_releases(&platform)
}

fn choose_release<'a>(
    releases: &'a [LoaderRelease],
    flavor: &str,
) -> Result<(&'a LoaderRelease, &'a ReleaseAsset), String> {
    let preferred_pre = flavor == "dev";
    for release in releases {
        if release.prerelease == preferred_pre {
            if let Some(asset) = release
                .assets
                .iter()
                .find(|asset| asset.name.to_ascii_lowercase().contains(flavor))
            {
                return Ok((release, asset));
            }
        }
    }
    Err(format!("no {flavor} loader asset was found"))
}

fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("loader-downloads");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create download cache: {e}"))?;
    Ok(dir)
}

fn download(url: &str, path: &Path) -> Result<String, String> {
    let response = ureq::get(url)
        .set("User-Agent", "kmod-launcher/0.1")
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let mut reader = response.into_reader();
    let mut file =
        fs::File::create(path).map_err(|e| format!("cannot create download file: {e}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 1024 * 64];
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(|e| format!("download read failed: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])
            .map_err(|e| format!("download write failed: {e}"))?;
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn safe_zip_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    let relative = Path::new(name);
    if relative.is_absolute()
        || relative.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("unsafe path in loader archive: {name}"));
    }
    Ok(root.join(relative))
}

fn extract_windows(zip_path: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| format!("cannot create game directory: {e}"))?;
    let file = fs::File::open(zip_path).map_err(|e| format!("cannot open loader archive: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("invalid loader archive: {e}"))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let output = safe_zip_path(target, entry.name())?;
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&output)
            .map_err(|e| format!("cannot write {}: {e}", output.display()))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn install_loader(
    app: tauri::AppHandle,
    platform: String,
    flavor: String,
    target_path: Option<String>,
) -> Result<InstallResult, String> {
    if flavor != "dev" && flavor != "release" {
        return Err("flavor must be dev or release".into());
    }
    let releases = fetch_releases(&platform)?;
    let (release, asset) = choose_release(&releases, &flavor)?;
    let path = cache_dir(&app)?.join(&asset.name);
    let sha256 = download(&asset.download_url, &path)?;
    if platform == "windows" {
        let target = target_path
            .filter(|p| !p.trim().is_empty())
            .ok_or("Windows installation requires the KARDS\nBinaries\\Win64 directory")?;
        extract_windows(&path, Path::new(&target))?;
        Ok(InstallResult {
            platform,
            flavor,
            version: release.tag_name.clone(),
            asset_name: asset.name.clone(),
            path: target,
            installed: true,
            detail: "KMod Loader installed next to the KARDS executable".into(),
            sha256,
        })
    } else {
        Ok(InstallResult { platform, flavor, version: release.tag_name.clone(), asset_name: asset.name.clone(), path: path.to_string_lossy().into_owned(), installed: false, detail: "APK downloaded. Open the file with Android's package installer to finish installation.".into(), sha256 })
    }
}

#[tauri::command]
fn default_flavor() -> &'static str {
    "release"
}

#[tauri::command]
fn open_installer(app: tauri::AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_releases,
            install_loader,
            default_flavor,
            open_installer
        ])
        .run(tauri::generate_context!())
        .expect("error while running KMod Launcher");
}
