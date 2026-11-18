import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type Platform = "windows" | "android";
type Flavor = "release" | "dev";
type Asset = { name: string; downloadUrl: string; size: number };
type Release = { tagName: string; name: string; prerelease: boolean; publishedAt?: string; assets: Asset[] };
type InstallResult = { version: string; assetName: string; path: string; installed: boolean; detail: string; sha256: string; backupPath?: string };
type GameInstall = { path: string; label: string };

const platformLabel: Record<Platform, string> = { windows: "Windows", android: "Android" };

function detectPlatform(): Platform {
  return /android/i.test(navigator.userAgent) ? "android" : "windows";
}

export default function App() {
  const platform = useMemo(detectPlatform, []);
  const [flavor, setFlavor] = useState<Flavor>("release");
  const [releases, setReleases] = useState<Release[]>([]);
  const [target, setTarget] = useState("");
  const [detectedGames, setDetectedGames] = useState<GameInstall[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("正在读取 GitHub release…");
  const [result, setResult] = useState<InstallResult | null>(null);

  const refresh = async () => {
    setLoading(true); setMessage("正在读取 GitHub release…");
    try {
      const data = await invoke<Release[]>("list_releases", { platform });
      setReleases(data);
      setMessage(data.length ? "已连接到 GitHub" : "没有找到可用 release");
    } catch (error) { setMessage(String(error)); }
    finally { setLoading(false); }
  };

  useEffect(() => { void refresh(); }, [platform]);

  useEffect(() => {
    if (platform !== "windows") return;
    void invoke<GameInstall[]>("detect_windows_games").then((games) => {
      setDetectedGames(games);
      if (!target && games.length === 1) setTarget(games[0].path);
    }).catch(() => setDetectedGames([]));
  }, [platform]);

  const preferred = releases.find((release) => release.prerelease === (flavor === "dev") && release.assets.some((asset) => asset.name.toLowerCase().includes(flavor)))
    ?? releases.find((release) => release.assets.some((asset) => asset.name.toLowerCase().includes(flavor)));

  const chooseTarget = async () => {
    const selected = await open({ directory: true, multiple: false, title: "选择 KARDS\\Binaries\\Win64 目录" });
    if (typeof selected === "string") setTarget(selected);
  };

  const install = async () => {
    if (platform === "windows" && !target) { setMessage("请先选择 KARDS\\Binaries\\Win64 目录"); return; }
    setLoading(true); setResult(null); setMessage(`正在下载 ${flavor} loader…`);
    try {
      const installed = await invoke<InstallResult>("install_loader", { platform, flavor, targetPath: target || null });
      setResult(installed); setMessage(installed.detail);
    } catch (error) { setMessage(String(error)); }
    finally { setLoading(false); }
  };

  return <main className="shell">
    <header className="hero"><div className="mark">K</div><div><p className="eyebrow">KARDS MOD TOOLS</p><h1>KMod Launcher</h1><p className="subtitle">统一管理 Windows 与 Android 版本的 KMod Loader</p></div></header>
    <section className="panel">
      <div className="section-heading"><div><p className="eyebrow">INSTALLER</p><h2>安装 KMod Loader</h2></div><span className="platform">{platformLabel[platform]}</span></div>
      <div className="flavors"><button className={flavor === "release" ? "selected" : ""} onClick={() => setFlavor("release")}><strong>Release</strong><span>推荐，适合日常使用</span></button><button className={flavor === "dev" ? "selected dev" : "dev"} onClick={() => setFlavor("dev")}><strong>Dev</strong><span>开发版，包含调试功能</span></button></div>
      <div className="release-card"><div><span className="label">默认版本</span><h3>{preferred?.name || "等待 release 信息"}</h3><p>{preferred ? `${preferred.tagName} · ${preferred.assets.find((asset) => asset.name.toLowerCase().includes(flavor))?.name || "未匹配资产"}` : message}</p></div><button className="ghost" onClick={() => void refresh()} disabled={loading}>刷新</button></div>
      {platform === "windows" && <div className="path-block"><div className="path-row"><input value={target} onChange={(event) => setTarget(event.target.value)} placeholder="KARDS\\Binaries\\Win64 目录" /><button className="ghost" onClick={() => void chooseTarget()}>选择目录</button></div>{detectedGames.length > 0 && <div className="detected">自动发现：{detectedGames.map((game) => <button key={game.path} onClick={() => setTarget(game.path)}>{game.path}</button>)}</div>}</div>}
      <button className="primary" onClick={() => void install()} disabled={loading || !preferred}>{loading ? "处理中…" : platform === "android" ? "下载并准备安装 APK" : `安装 ${flavor} Loader`}</button>
      <p className="status">{message}</p>
      {result && <div className="result"><strong>{result.installed ? "安装完成" : "APK 已准备好"}</strong><span>{result.path}</span><small>SHA-256 {result.sha256}</small>{result.backupPath && <small>原有 dwmapi.dll 已备份到 {result.backupPath}</small>}</div>}
      {result && !result.installed && <button className="ghost installer" onClick={() => void invoke("open_installer", { path: result.path })}>打开系统安装器</button>}
    </section>
    <section className="notes"><div><span>Windows</span><p>将代理 DLL 与 kmod 文件夹安装到游戏目录旁。</p></div><div><span>Android</span><p>下载后交给系统安装器完成 APK 安装，应用数据目录由 KMod 管理。</p></div></section>
    <footer>默认使用最新 release · Release 来源：GitHub</footer>
  </main>;
}
