use std::fs;
use std::path::Path;

use crate::{
    error::{AppError, Result},
    models::{BrowserRuntime, Environment, Proxy},
    paths,
};

const TEMPLATE: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>@@TITLE@@</title>
<style>
  :root {
    --accent: @@ACCENT@@;
    --accent-width: @@WIDTH@@px;
    --bg: #f4f6fb;
    --bg-soft: #ffffff;
    --bg-muted: #eef1f8;
    --text: #121826;
    --text-soft: #4f5b70;
    --text-hint: #7b879b;
    --line: #e0e5ef;
    --shadow: 0 18px 60px rgba(22, 32, 52, 0.08);
    --ok: #16a34a;
    --warn: #d97706;
    --bad: #dc2626;
    --radius: 18px;
  }
  * { box-sizing: border-box; }
  html, body { min-height: 100%; }
  body {
    margin: 0;
    color: var(--text);
    background:
      radial-gradient(900px 520px at 8% -10%, color-mix(in srgb, var(--accent) 20%, transparent), transparent 70%),
      radial-gradient(760px 480px at 100% 0%, color-mix(in srgb, var(--accent) 12%, transparent), transparent 72%),
      var(--bg);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", "PingFang SC", Roboto, sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  body::before {
    content: "";
    position: fixed;
    inset: 0 auto auto 0;
    width: 100%;
    height: var(--accent-width);
    background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 45%, #ffffff));
    z-index: 20;
  }
  .page { position: relative; max-width: 1280px; margin: 0 auto; padding: 38px 36px 30px; }
  .topbar { display: flex; align-items: center; justify-content: space-between; gap: 18px; margin-bottom: 26px; }
  .brand { display: flex; align-items: center; gap: 12px; }
  .logo {
    width: 42px; height: 42px; border-radius: 13px; display: grid; place-items: center;
    color: #fff; background: linear-gradient(145deg, var(--accent), color-mix(in srgb, var(--accent) 55%, #111827));
    box-shadow: 0 10px 24px color-mix(in srgb, var(--accent) 34%, transparent);
  }
  .brand-name { font-size: 16px; font-weight: 750; letter-spacing: .2px; }
  .brand-sub { font-size: 10px; letter-spacing: .22em; color: var(--text-hint); text-transform: uppercase; margin-top: 2px; }
  .top-right { display: flex; align-items: center; gap: 10px; font-size: 12px; color: var(--text-hint); }
  .ready {
    display: inline-flex; align-items: center; gap: 7px; padding: 7px 12px; border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--ok) 26%, var(--line)); color: var(--ok);
    background: color-mix(in srgb, var(--ok) 8%, transparent); font-weight: 650;
  }
  .pulse { width: 7px; height: 7px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 0 4px color-mix(in srgb, var(--ok) 16%, transparent); }
  .hero {
    display: grid; grid-template-columns: minmax(0, 1.45fr) minmax(320px, .8fr); gap: 22px;
    background: var(--bg-soft); border: 1px solid var(--line); border-left: var(--accent-width) solid var(--accent);
    border-radius: var(--radius); box-shadow: var(--shadow); overflow: hidden;
  }
  .hero-main { padding: 32px 34px 30px; min-width: 0; }
  .eyebrow { font-size: 10px; letter-spacing: .2em; color: var(--accent); font-weight: 750; text-transform: uppercase; }
  h1 { margin: 10px 0 10px; font-size: clamp(28px, 3.2vw, 44px); line-height: 1.05; letter-spacing: -1.2px; }
  .chips { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 12px; }
  .chip {
    display: inline-flex; align-items: center; gap: 5px; padding: 4px 9px; border-radius: 999px;
    font-size: 11px; color: var(--text-soft); background: var(--bg-muted); border: 1px solid var(--line);
  }
  .chip.accent { color: var(--accent); border-color: color-mix(in srgb, var(--accent) 34%, var(--line)); background: color-mix(in srgb, var(--accent) 8%, transparent); }
  .chip.ok { color: var(--ok); border-color: color-mix(in srgb, var(--ok) 26%, var(--line)); background: color-mix(in srgb, var(--ok) 7%, transparent); }
  .notes { margin: 16px 0 0; color: var(--text-soft); font-size: 12px; line-height: 1.7; white-space: pre-wrap; max-height: 76px; overflow: auto; }
  .address { margin-top: 22px; padding: 12px 13px; border-radius: 12px; border: 1px dashed color-mix(in srgb, var(--accent) 34%, var(--line)); background: color-mix(in srgb, var(--accent) 4%, transparent); }
  .address-label { display: flex; justify-content: space-between; gap: 10px; font-size: 10px; color: var(--text-hint); letter-spacing: .06em; }
  .address code { display: block; margin-top: 5px; font-family: "Cascadia Code", Consolas, monospace; font-size: 11px; line-height: 1.55; color: var(--text-soft); word-break: break-all; }
  .copy { border: 0; background: transparent; color: var(--accent); font-size: 10px; cursor: pointer; padding: 0; }
  .addr-actions { display: inline-flex; align-items: center; gap: 10px; }
  .go-form { display: flex; gap: 8px; margin-top: 18px; }
  .go-form input {
    flex: 1; min-width: 0; height: 42px; border-radius: 11px; border: 1px solid var(--line);
    background: var(--bg-soft); color: var(--text); padding: 0 13px; font-size: 12px; outline: none;
  }
  .go-form input:focus { border-color: var(--accent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 13%, transparent); }
  .go-form button {
    height: 42px; padding: 0 17px; border: 0; border-radius: 11px; cursor: pointer; font-weight: 700; font-size: 12px;
    color: #fff; background: var(--accent); box-shadow: 0 8px 18px color-mix(in srgb, var(--accent) 28%, transparent);
  }
  .hint { margin: 9px 0 0; font-size: 10px; color: var(--text-hint); }
  .hero-side { padding: 26px; background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 6%, var(--bg-soft)), var(--bg-soft)); border-left: 1px solid var(--line); }
  .ip-card { border-radius: 14px; padding: 18px; color: #fff; background: linear-gradient(135deg, var(--accent), color-mix(in srgb, var(--accent) 50%, #0f172a)); box-shadow: 0 16px 30px color-mix(in srgb, var(--accent) 24%, transparent); }
  .ip-card .label { font-size: 10px; letter-spacing: .13em; opacity: .82; text-transform: uppercase; }
  .ip-card strong { display: block; margin: 8px 0 5px; font-family: "Cascadia Code", Consolas, monospace; font-size: 24px; letter-spacing: -.5px; word-break: break-all; }
  .ip-card .meta { font-size: 11px; opacity: .86; }
  .ip-card .source { display: inline-block; margin-top: 11px; padding: 3px 8px; border-radius: 999px; font-size: 10px; background: rgba(255,255,255,.18); }
  .mini { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 13px; }
  .mini div { padding: 11px 12px; border-radius: 11px; border: 1px solid var(--line); background: color-mix(in srgb, var(--bg-soft) 70%, transparent); min-width: 0; }
  .mini span { display: block; font-size: 9px; color: var(--text-hint); letter-spacing: .05em; }
  .mini b { display: block; margin-top: 4px; font-size: 11px; font-weight: 650; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .runtime { margin-top: 13px; padding: 10px 12px; border-radius: 10px; background: var(--bg-muted); color: var(--text-soft); font-size: 10px; line-height: 1.6; }
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-top: 18px; }
  .stat { padding: 17px 18px; border-radius: 15px; border: 1px solid var(--line); background: var(--bg-soft); box-shadow: 0 8px 30px rgba(22,32,52,.04); }
  .stat .label { font-size: 10px; color: var(--text-hint); }
  .stat .value { margin-top: 7px; font-size: 24px; font-weight: 760; letter-spacing: -.7px; }
  .stat .hint { margin-top: 3px; font-size: 10px; color: var(--text-hint); }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-top: 16px; }
  .card { border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg-soft); padding: 22px 23px; box-shadow: 0 8px 30px rgba(22,32,52,.04); min-width: 0; }
  .card h2 { display: flex; align-items: center; justify-content: flex-start; gap: 8px; margin: 0 0 14px; font-size: 13px; letter-spacing: .01em; }
  .card h2::before { content: ""; width: 3px; height: 14px; border-radius: 2px; background: var(--accent); flex: 0 0 auto; }
  .card h2 span.live { margin-left: auto; padding: 2px 7px; border-radius: 999px; font-size: 9px; letter-spacing: .1em; color: var(--ok); background: color-mix(in srgb, var(--ok) 9%, transparent); }
  .rows { display: grid; gap: 0; }
  .row { display: grid; grid-template-columns: 118px minmax(0, 1fr); gap: 12px; padding: 8px 0; border-bottom: 1px dashed var(--line); font-size: 11px; }
  .row:last-child { border-bottom: 0; }
  .row dt { color: var(--text-hint); }
  .row dd { margin: 0; color: var(--text); word-break: break-word; }
  .row dd.mono { font-family: "Cascadia Code", Consolas, monospace; font-size: 10.5px; }
  .mono { font-family: "Cascadia Code", Consolas, monospace; }
  .ok { color: var(--ok); }
  .bad { color: var(--bad); }
  .footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-top: 18px; padding: 0 4px; font-size: 10px; color: var(--text-hint); }
  .footer span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  @media (max-width: 980px) {
    .page { padding: 26px 18px; }
    .hero { grid-template-columns: 1fr; }
    .hero-side { border-left: 0; border-top: 1px solid var(--line); }
    .stats { grid-template-columns: 1fr 1fr; }
    .grid { grid-template-columns: 1fr; }
  }
  @media (max-width: 580px) {
    .topbar { align-items: flex-start; }
    .top-right .date { display: none; }
    .hero-main { padding: 24px 20px; }
    .hero-side { padding: 20px; }
    .stats { grid-template-columns: 1fr; }
    .row { grid-template-columns: 96px minmax(0, 1fr); }
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg: #0b1018; --bg-soft: #121a26; --bg-muted: #1a2432; --text: #eef3fb;
      --text-soft: #b5c0d2; --text-hint: #7f8da3; --line: #263244;
      --shadow: 0 18px 60px rgba(0,0,0,.32);
    }
    body {
      background:
        radial-gradient(900px 520px at 8% -10%, color-mix(in srgb, var(--accent) 18%, transparent), transparent 70%),
        radial-gradient(760px 480px at 100% 0%, color-mix(in srgb, var(--accent) 10%, transparent), transparent 72%),
        var(--bg);
    }
    .hero-side { background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 9%, var(--bg-soft)), var(--bg-soft)); }
    .mini div { background: color-mix(in srgb, var(--bg-soft) 80%, transparent); }
  }
</style>
</head>
<body>
<div class="page">
  <header class="topbar">
    <div class="brand">
      <div class="logo" aria-hidden="true">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
          <path d="M12 3a9 9 0 0 0-9 9v2a4 4 0 0 0 8 0v-2a4 4 0 0 1 8 0v3a3 3 0 0 1-6 0v-2"/>
          <path d="M21 12a9 9 0 0 0-9-9"/>
          <path d="M6 18a3 3 0 0 0 3-3v-3"/>
        </svg>
      </div>
      <div>
        <div class="brand-name">FinEnv</div>
        <div class="brand-sub">Fingerprint Environment</div>
      </div>
    </div>
    <div class="top-right">
      <span class="ready"><span class="pulse"></span> 环境已就绪</span>
      <span class="date" id="clock"></span>
    </div>
  </header>

  <section class="hero">
    <div class="hero-main">
      <div class="eyebrow">Isolated Browser · Ready</div>
      <h1>@@NAME@@</h1>
      <div class="chips">@@CHIPS@@</div>
      @@NOTES@@
      <div class="address">
        <div class="address-label"><span>启动后访问地址</span><span class="addr-actions"><button class="copy" type="button" id="reload-page">刷新信息</button><button class="copy" type="button" id="copy-url">复制地址</button></span></div>
        <code id="start-url">@@URL@@</code>
      </div>
      <form class="go-form" id="go-form" autocomplete="off">
        <input id="go-url" type="text" inputmode="url" placeholder="输入测试地址，例如 https://test.example.com/login" aria-label="测试地址">
        <button type="submit">访问地址</button>
      </form>
      <p class="hint">也可以直接使用浏览器地址栏；按 <b>/</b> 聚焦输入框。</p>
    </div>
    <aside class="hero-side">
      <div class="ip-card">
        <div class="label">当前出口 IP</div>
        <strong>@@IP@@</strong>
        <div class="meta">@@IP_LOCATION@@</div>
        <span class="source">@@IP_SOURCE@@</span>
      </div>
      <div class="mini">
        <div><span>代理</span><b title="@@PROXY_TITLE@@">@@PROXY@@</b></div>
        <div><span>指纹平台</span><b>@@FP_PLATFORM@@</b></div>
        <div><span>时区</span><b>@@TZ@@</b></div>
        <div><span>屏幕</span><b>@@SCREEN@@</b></div>
      </div>
      <div class="runtime">@@RUNTIME@@</div>
    </aside>
  </section>

  <section class="stats">@@STATS@@</section>

  <section class="grid">
    <article class="card">
      <h2>环境信息</h2>
      <dl class="rows">@@ENV_ROWS@@</dl>
    </article>
    <article class="card">
      <h2>指纹摘要</h2>
      <dl class="rows">@@FP_ROWS@@</dl>
    </article>
    <article class="card">
      <h2>网络与代理</h2>
      <dl class="rows">@@NET_ROWS@@</dl>
    </article>
    <article class="card">
      <h2>实时浏览器信息 <span class="live">LIVE</span></h2>
      <dl class="rows">
        <div class="row"><dt>User Agent</dt><dd class="mono" data-live="ua">-</dd></div>
        <div class="row"><dt>Platform</dt><dd data-live="platform">-</dd></div>
        <div class="row"><dt>语言</dt><dd data-live="languages">-</dd></div>
        <div class="row"><dt>时区</dt><dd data-live="timezone">-</dd></div>
        <div class="row"><dt>屏幕</dt><dd data-live="screen">-</dd></div>
        <div class="row"><dt>CPU / 内存</dt><dd data-live="hardware">-</dd></div>
        <div class="row"><dt>WebGL</dt><dd class="mono" data-live="webgl">-</dd></div>
        <div class="row"><dt>自动化标记</dt><dd data-live="webdriver">-</dd></div>
      </dl>
    </article>
  </section>

  <footer class="footer">
    <span>本页由 FinEnv 在本地生成，不依赖外部网络 · 启动页会随环境信息更新</span>
    <span>@@GENERATED@@ · @@ENV_SHORT@@</span>
  </footer>
</div>
<script>
(function () {
  const pad = (n) => String(n).padStart(2, '0');
  function tick() {
    const d = new Date();
    const el = document.getElementById('clock');
    if (el) el.textContent = d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate()) + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds());
  }
  tick();
  setInterval(tick, 1000);

  const pick = (name) => document.querySelector('[data-live="' + name + '"]');
  const set = (name, value) => { const el = pick(name); if (el) el.textContent = value || '-'; };

  set('ua', navigator.userAgent);
  set('platform', navigator.platform);
  set('languages', (navigator.languages || [navigator.language]).join(', '));
  try { set('timezone', Intl.DateTimeFormat().resolvedOptions().timeZone); } catch (_) {}
  set('screen', screen.width + ' × ' + screen.height + ' @ ' + (window.devicePixelRatio || 1) + 'x');
  set('hardware', (navigator.hardwareConcurrency || '?') + ' 核 / ' + (navigator.deviceMemory || '?') + ' GB');
  try {
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    if (gl) {
      const ext = gl.getExtension('WEBGL_debug_renderer_info');
      const vendor = ext ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR);
      const renderer = ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
      set('webgl', vendor + ' · ' + renderer);
    } else {
      set('webgl', 'WebGL 不可用');
    }
  } catch (_) { set('webgl', '读取失败'); }
  const wd = pick('webdriver');
  if (wd) {
    wd.textContent = navigator.webdriver ? 'webdriver = true' : 'webdriver = false';
    wd.className = navigator.webdriver ? 'bad' : 'ok';
  }

  const form = document.getElementById('go-form');
  const input = document.getElementById('go-url');
  form.addEventListener('submit', function (event) {
    event.preventDefault();
    let value = input.value.trim();
    if (!value) { input.focus(); return; }
    if (!/^https?:\/\//i.test(value)) value = 'https://' + value;
    try {
      const parsed = new URL(value);
      if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') throw new Error('unsupported');
      location.href = parsed.toString();
    } catch (_) {
      input.setCustomValidity('请输入有效的 http/https 地址');
      input.reportValidity();
      setTimeout(function () { input.setCustomValidity(''); }, 1500);
    }
  });
  document.addEventListener('keydown', function (event) {
    if (event.key === '/' && document.activeElement !== input) {
      event.preventDefault();
      input.focus();
    }
  });
  const copy = document.getElementById('copy-url');
  document.getElementById('reload-page').addEventListener('click', function () { location.reload(); });
  copy.addEventListener('click', function () {
    const text = document.getElementById('start-url').textContent || location.href;
    const done = function () { copy.textContent = '已复制'; setTimeout(function () { copy.textContent = '复制地址'; }, 1400); };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done).catch(function () { window.prompt('复制启动地址', text); });
    } else {
      window.prompt('复制启动地址', text);
    }
  });
})();
</script>
</body>
</html>
"#;

pub fn file_url(path: &Path) -> Result<String> {
    url::Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|_| AppError::internal(format!("无法生成启动页地址: {}", path.display())))
}

pub fn write_at(
    path: &Path,
    env: &Environment,
    proxy: Option<&Proxy>,
    account_count: u32,
    snapshot_count: u32,
    runtime: Option<&BrowserRuntime>,
    start_url: &str,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        paths::ensure_dir(parent)?;
    }
    let html = render(
        env,
        proxy,
        account_count,
        snapshot_count,
        runtime,
        start_url,
    );
    fs::write(path, html)?;
    Ok(())
}

pub fn refresh_if_present(
    data_dir: &Path,
    env: &Environment,
    proxy: Option<&Proxy>,
    account_count: u32,
    snapshot_count: u32,
    runtime: Option<&BrowserRuntime>,
) -> Result<()> {
    let path = paths::start_page_path(data_dir, &env.id);
    if !path.exists() {
        return Ok(());
    }
    let url = file_url(&path)?;
    write_at(
        &path,
        env,
        proxy,
        account_count,
        snapshot_count,
        runtime,
        &url,
    )
}

fn render(
    env: &Environment,
    proxy: Option<&Proxy>,
    account_count: u32,
    snapshot_count: u32,
    runtime: Option<&BrowserRuntime>,
    start_url: &str,
) -> String {
    let accent = normalize_accent(&env.border_color, &env.id);
    let width = env.border_width.clamp(1, 12);
    let test_env = match env.test_env.as_str() {
        "dev" => "开发",
        "test" => "测试",
        "pre" => "预发",
        "prod" => "生产",
        other if !other.is_empty() => other,
        _ => "未指定",
    };
    let status = match env.status.as_str() {
        "disabled" => "已停用",
        "archived" => "已归档",
        _ => "已启用",
    };

    let mut chips = vec![
        chip(status, if env.status == "active" { "ok" } else { "" }),
        chip(test_env, "accent"),
    ];
    if !env.account_role.is_empty() {
        chips.push(chip(&env.account_role, ""));
    }
    if !env.version.is_empty() {
        chips.push(chip(&env.version, ""));
    }
    for tag in env.tags.iter().take(6) {
        chips.push(chip(&format!("#{tag}"), ""));
    }
    if !env.case_id.is_empty() {
        chips.push(chip(&env.case_id, ""));
    }

    let notes = if env.notes.trim().is_empty() {
        String::new()
    } else {
        format!("<p class=\"notes\">{}</p>", esc(&env.notes))
    };

    let ip = env.ip_address.as_deref().unwrap_or("未检测");
    let ip_location = [
        env.ip_city.as_deref(),
        env.ip_country.as_deref().or(env.ip_country_code.as_deref()),
        env.ip_timezone.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|v| !v.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let ip_source = match env.ip_source.as_deref() {
        Some("proxy") => "经代理出口",
        Some("direct") => "直连出口",
        _ if env.ip_address.is_none() => "检测中，刷新本页查看结果",
        _ => "未记录来源",
    };
    let proxy_label = proxy
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "直连".into());
    let proxy_title = proxy
        .map(|p| format!("{}://{}:{}", p.protocol, p.host, p.port))
        .unwrap_or_else(|| "未绑定代理".into());
    let runtime_text = match runtime {
        Some(rt) => format!(
            "PID {} · 调试端口 {} · 运行中<br>内存 {} MB · CPU {:.1}%",
            rt.pid, rt.debug_port, rt.memory_mb, rt.cpu_percent
        ),
        None => "浏览器启动中…".to_string(),
    };

    let stats = format!(
        "{}{}{}{}",
        stat("账号台账", &account_count.to_string(), "密码本地加密"),
        stat("指纹快照", &snapshot_count.to_string(), "可一键还原"),
        stat(
            "浏览器进程",
            runtime
                .map(|r| r.pid.to_string())
                .as_deref()
                .unwrap_or("启动中"),
            "独立 profile"
        ),
        stat(
            "实时内存",
            &runtime
                .map(|r| format!("{} MB", r.memory_mb))
                .unwrap_or_else(|| "-".into()),
            "独立进程"
        ),
    );

    let env_rows = [
        row("环境 ID", &env.id, true),
        row("项目", fallback(&env.project), false),
        row("测试环境", test_env, false),
        row("账号角色", fallback(&env.account_role), false),
        row("版本", fallback(&env.version), false),
        row("用例 ID", fallback(&env.case_id), true),
        row("内核", &env.kernel, false),
        row(
            "最近打开",
            &pretty_time(env.last_opened_at.as_deref()),
            false,
        ),
    ]
    .concat();

    let fp = &env.fingerprint;
    let fp_rows = [
        row("User Agent", &fp.user_agent, true),
        row("Platform", &fp.platform, false),
        row("语言", &fp.languages.join(", "), false),
        row("时区", &fp.timezone, false),
        row(
            "屏幕",
            &format!(
                "{}×{} · DPR {}",
                fp.screen.width, fp.screen.height, fp.screen.pixel_ratio
            ),
            false,
        ),
        row(
            "WebGL",
            &format!("{} · {}", fp.webgl.vendor, fp.webgl.renderer),
            true,
        ),
        row(
            "Canvas / Audio",
            &format!("{} / {}", fp.canvas.mode, fp.audio.mode),
            false,
        ),
        row(
            "WebRTC",
            &format!(
                "{}{}",
                fp.webrtc.mode,
                fp.webrtc
                    .public_ip
                    .as_deref()
                    .map(|ip| format!(" · {ip}"))
                    .unwrap_or_default()
            ),
            false,
        ),
        row("Client Hints", &fp.client_hints.platform, false),
        row(
            "字体 / 插件",
            &format!("{} 项 / {} 项", fp.fonts.len(), fp.plugins.len()),
            false,
        ),
    ]
    .concat();

    let net_rows = [
        row("代理", &proxy_label, false),
        row("代理地址", &proxy_title, true),
        row("出口 IP", ip, true),
        row(
            "位置",
            if ip_location.is_empty() {
                "尚未检测"
            } else {
                &ip_location
            },
            false,
        ),
        row("来源", ip_source, false),
        row(
            "最近检测",
            &pretty_time(env.ip_checked_at.as_deref()),
            false,
        ),
        row(
            "代理出口",
            proxy.and_then(|p| p.exit_ip.as_deref()).unwrap_or("—"),
            true,
        ),
        row(
            "延迟",
            &proxy
                .and_then(|p| p.latency_ms)
                .map(|ms| format!("{ms} ms"))
                .unwrap_or_else(|| "—".into()),
            false,
        ),
    ]
    .concat();

    let title = format!("{} · FinEnv 环境启动页", env.name);
    let generated = format!("生成于 {}", pretty_time(Some(&crate::models::now_iso())));
    let env_short = if env.id.len() > 12 {
        format!("{}…", &env.id[..12])
    } else {
        env.id.clone()
    };

    TEMPLATE
        .replace("@@TITLE@@", &esc(&title))
        .replace("@@ACCENT@@", &accent)
        .replace("@@WIDTH@@", &width.to_string())
        .replace("@@NAME@@", &esc(&env.name))
        .replace("@@CHIPS@@", &chips.concat())
        .replace("@@NOTES@@", &notes)
        .replace("@@URL@@", &esc(start_url))
        .replace("@@IP@@", &esc(ip))
        .replace(
            "@@IP_LOCATION@@",
            &esc(if ip_location.is_empty() {
                "尚未检测出口位置"
            } else {
                &ip_location
            }),
        )
        .replace("@@IP_SOURCE@@", ip_source)
        .replace("@@PROXY@@", &esc(&proxy_label))
        .replace("@@PROXY_TITLE@@", &esc(&proxy_title))
        .replace("@@FP_PLATFORM@@", &esc(&fp.client_hints.platform))
        .replace("@@TZ@@", &esc(&fp.timezone))
        .replace(
            "@@SCREEN@@",
            &esc(&format!("{}×{}", fp.screen.width, fp.screen.height)),
        )
        .replace("@@RUNTIME@@", &runtime_text)
        .replace("@@STATS@@", &stats)
        .replace("@@ENV_ROWS@@", &env_rows)
        .replace("@@FP_ROWS@@", &fp_rows)
        .replace("@@NET_ROWS@@", &net_rows)
        .replace("@@GENERATED@@", &esc(&generated))
        .replace("@@ENV_SHORT@@", &esc(&env_short))
}

fn normalize_accent(value: &str, env_id: &str) -> String {
    let trimmed = value.trim().trim_start_matches('#');
    if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return format!("#{}", trimmed.to_ascii_uppercase());
    }
    const PALETTE: [&str; 10] = [
        "#2563EB", "#7C3AED", "#DB2777", "#DC2626", "#EA580C", "#CA8A04", "#16A34A", "#0D9488",
        "#0891B2", "#475569",
    ];
    let hash = env_id.bytes().fold(0u32, |acc, value| {
        acc.wrapping_mul(31).wrapping_add(value as u32)
    });
    PALETTE[(hash as usize) % PALETTE.len()].to_string()
}

fn chip(text: &str, class: &str) -> String {
    format!("<span class=\"chip {class}\">{}</span>", esc(text))
}

fn stat(label: &str, value: &str, hint: &str) -> String {
    format!(
        "<div class=\"stat\"><div class=\"label\">{}</div><div class=\"value\">{}</div><div class=\"hint\">{}</div></div>",
        esc(label),
        esc(value),
        esc(hint)
    )
}

fn row(label: &str, value: &str, mono: bool) -> String {
    format!(
        "<div class=\"row\"><dt>{}</dt><dd class=\"{}\">{}</dd></div>",
        esc(label),
        if mono { "mono" } else { "" },
        esc(value)
    )
}

fn fallback(value: &str) -> &str {
    if value.trim().is_empty() {
        "未指定"
    } else {
        value
    }
}

fn pretty_time(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    value
        .replace('T', " ")
        .split('.')
        .next()
        .unwrap_or(value)
        .trim_end_matches('Z')
        .to_string()
}

fn esc(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{new_id, now_iso, Environment};

    fn sample_env() -> Environment {
        Environment {
            id: new_id(),
            name: "Test <Env>".into(),
            group_id: None,
            status: "active".into(),
            profile_dir: String::new(),
            proxy_id: None,
            fingerprint_snapshot_id: None,
            project: "交易".into(),
            test_env: "test".into(),
            account_role: "运营".into(),
            version: "V1".into(),
            case_id: "CASE-1".into(),
            tags: vec!["p0".into()],
            notes: "<script>alert(1)</script>".into(),
            kernel: "chromium".into(),
            chromium_path: None,
            chromium_launch_args: Vec::new(),
            taskbar_badge_text: String::new(),
            auto_restart: false,
            memory_limit_mb: None,
            idle_timeout_min: None,
            fingerprint: crate::fingerprint::template("win11"),
            border_color: "#16A34A".into(),
            border_width: 5,
            ip_address: Some("203.0.113.10".into()),
            ip_country: Some("Japan".into()),
            ip_country_code: Some("JP".into()),
            ip_city: Some("Tokyo".into()),
            ip_timezone: Some("Asia/Tokyo".into()),
            ip_source: Some("proxy".into()),
            ip_checked_at: Some(now_iso()),
            created_at: now_iso(),
            updated_at: now_iso(),
            last_opened_at: None,
        }
    }

    #[test]
    fn renders_environment_information_and_escapes_html() {
        let env = sample_env();
        let url = "file:///tmp/start.html";
        let html = render(&env, None, 2, 3, None, url);
        assert!(html.contains("Test &lt;Env&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("203.0.113.10"));
        assert!(html.contains("--accent: #16A34A"));
        assert!(html.contains("--accent-width: 5px"));
        assert!(html.contains(url));
    }
}
