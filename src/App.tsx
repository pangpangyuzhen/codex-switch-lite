import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  BadgeCheck,
  CircleAlert,
  Cloud,
  KeyRound,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  Sparkles,
} from "lucide-react";

type Mode = "plus" | "external" | "unknown";

type Status = {
  initialized: boolean;
  mode: Mode;
  live_provider: string | null;
  auth_exists: boolean;
  key_saved: boolean;
  routing_ok: boolean;
  config_path: string;
  last_backup: string | null;
  warning: string | null;
};

type ActionResult = {
  ok: boolean;
  message: string;
};

const modeLabel = (mode: Mode) =>
  mode === "plus" ? "ChatGPT Plus" : mode === "external" ? "External API" : "未识别";

export default function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const s = await invoke<Status>("get_status");
      setStatus(s);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const run = async (fn: () => Promise<ActionResult>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const result = await fn();
      if (!result.ok) throw new Error(result.message);
      setNotice(result.message);
      setApiKey("");
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const initialize = () => run(() => invoke<ActionResult>("initialize_profiles"));
  const saveKey = () => {
    if (!apiKey.trim()) {
      setError("请先填写 External API Key");
      return;
    }
    return run(() => invoke<ActionResult>("save_external_key", { key: apiKey.trim() }));
  };
  const switchMode = (mode: "plus" | "external") =>
    run(() => invoke<ActionResult>("switch_mode", { mode }));
  const restore = () => run(() => invoke<ActionResult>("restore_latest_backup"));

  const externalReady = useMemo(
    () => Boolean(status?.initialized && status.key_saved),
    [status],
  );

  return (
    <main className="shell">
      <header className="header">
        <div>
          <div className="eyebrow">CODEX ONLY · CC SWITCH LITE</div>
          <h1>Codex Switch Lite</h1>
          <p>只管两件事：Plus 与 External API。OAuth 永远不动。</p>
        </div>
        <div className={`mode-pill ${status?.mode ?? "unknown"}`}>
          <span className="mode-dot" />
          当前：{modeLabel(status?.mode ?? "unknown")}
        </div>
      </header>

      {!status?.initialized && (
        <section className="setup-card">
          <div className="setup-copy">
            <ShieldCheck size={22} />
            <div>
              <strong>第一次使用先初始化</strong>
              <span>会导入当前 Codex/旧 Skill 配置并建立 Plus / External 两套模式档案。</span>
            </div>
          </div>
          <button className="primary" disabled={busy} onClick={initialize}>
            初始化配置
          </button>
        </section>
      )}

      <section className="grid">
        <article className={`mode-card ${status?.mode === "plus" ? "active" : ""}`}>
          <div className="card-top">
            <div className="icon-box"><Sparkles size={22} /></div>
            {status?.mode === "plus" && <span className="active-tag">正在使用</span>}
          </div>
          <h2>ChatGPT Plus</h2>
          <p>官方 Codex Provider + 你已经登录的 ChatGPT OAuth。</p>
          <div className="facts">
            <div><span>Provider</span><b>openai</b></div>
            <div><span>OAuth</span><b>{status?.auth_exists ? "已保留" : "未检测到"}</b></div>
            <div><span>auth.json</span><b>永不写入</b></div>
          </div>
          <button
            className="switch-btn"
            disabled={busy || !status?.initialized || status?.mode === "plus"}
            onClick={() => switchMode("plus")}
          >
            切回 ChatGPT Plus
          </button>
        </article>

        <article className={`mode-card external ${status?.mode === "external" ? "active" : ""}`}>
          <div className="card-top">
            <div className="icon-box"><Cloud size={22} /></div>
            {status?.mode === "external" && <span className="active-tag">正在使用</span>}
          </div>
          <h2>External API</h2>
          <p>模型请求直接走外部 Responses API，官方账号只保留登录态。</p>
          <div className="facts">
            <div><span>Endpoint</span><b>code.adinsolution.link</b></div>
            <div><span>Model</span><b>gpt-5.5</b></div>
            <div><span>Key</span><b>{status?.key_saved ? "Keychain 已保存" : "未设置"}</b></div>
          </div>
          <button
            className="switch-btn"
            disabled={busy || !externalReady || status?.mode === "external"}
            onClick={() => switchMode("external")}
          >
            切到 External API
          </button>
        </article>
      </section>

      <section className="key-card">
        <div className="key-heading">
          <div className="icon-box small"><KeyRound size={18} /></div>
          <div>
            <strong>External API Key</strong>
            <span>只保存在 macOS Keychain；External 模式生效时才投影到 live config。</span>
          </div>
        </div>
        <div className="key-row">
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={status?.key_saved ? "•••••••••• 已保存，输入新 Key 可覆盖" : "粘贴 External API Key"}
            autoComplete="off"
          />
          <button className="secondary" disabled={busy} onClick={saveKey}>保存 Key</button>
        </div>
      </section>

      <section className="status-card">
        <div className="status-head">
          <strong>配置状态</strong>
          <button className="ghost" disabled={busy} onClick={() => void refresh()}>
            <RefreshCw size={15} /> 刷新
          </button>
        </div>
        <div className="status-lines">
          <div>
            <span>Live Provider</span>
            <b>{status?.live_provider ?? "—"}</b>
          </div>
          <div>
            <span>外部路由闭环</span>
            <b className={status?.routing_ok ? "ok" : "muted"}>
              {status?.routing_ok ? "已校验" : "未启用"}
            </b>
          </div>
          <div>
            <span>最近备份</span>
            <b title={status?.last_backup ?? ""}>{status?.last_backup ? "已存在" : "—"}</b>
          </div>
        </div>
        <div className="status-footer">
          <span className="path">{status?.config_path ?? "~/.codex/config.toml"}</span>
          <button className="ghost danger" disabled={busy || !status?.last_backup} onClick={restore}>
            <RotateCcw size={15} /> 恢复最近备份
          </button>
        </div>
      </section>

      {status?.warning && (
        <div className="banner warn"><CircleAlert size={18} /><span>{status.warning}</span></div>
      )}
      {notice && (
        <div className="banner success"><BadgeCheck size={18} /><span>{notice}</span></div>
      )}
      {error && (
        <div className="banner error"><CircleAlert size={18} /><span>{error}</span></div>
      )}

      <footer>
        切换会自动重启 Codex Desktop。重启后请新建聊天；旧线程不会原地换 Provider。
      </footer>
    </main>
  );
}
