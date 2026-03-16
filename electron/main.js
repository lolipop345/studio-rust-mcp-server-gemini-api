const { app, BrowserWindow, safeStorage, ipcMain, shell, dialog } = require("electron");
const path = require("path");
const fs = require("fs");
const https = require("https");
const http = require("http");
const querystring = require("querystring");

let mainWindow;

app.setName("GeminiStudio");
if (process.platform === 'darwin') {
  app.dock.setIcon(path.join(__dirname, 'icons', 'AppLogo.png'));
}

// Encrypted API key storage path
const keyFilePath = path.join(app.getPath("userData"), "api_key.enc");
// Google OAuth token storage path
const googleTokenFilePath = path.join(app.getPath("userData"), "google_token.enc");

// Load OAuth config (client_id / client_secret from oauth_config.json)
let oauthConfig = { client_id: "", client_secret: "" };
const oauthConfigPath = path.join(__dirname, "oauth_config.json");
if (fs.existsSync(oauthConfigPath)) {
  try { oauthConfig = JSON.parse(fs.readFileSync(oauthConfigPath, "utf-8")); } catch { }
}

// Active OAuth loopback port (set during google-oauth-start, used in exchange)
let oauthCallbackPort = null;

// Helper: send message to renderer
function sendToRenderer(channel, data) {
  if (mainWindow && mainWindow.webContents) {
    mainWindow.webContents.send(channel, data);
  }
}

function createWindow() {
  const iconPath = process.platform === 'win32'
    ? path.join(__dirname, 'icons', 'AppLogo.ico')
    : path.join(__dirname, 'icons', 'AppLogo.png');

  const isMac = process.platform === 'darwin';
  const isWin = process.platform === 'win32';

  const winOptions = {
    width: 1050,
    height: 750,
    minWidth: 600,
    minHeight: 500,
    title: "GeminiStudio",
    icon: iconPath,
    show: false,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
    },
  };

  if (isMac) {
    winOptions.titleBarStyle = "hiddenInset";
    winOptions.trafficLightPosition = { x: 16, y: 16 };
    winOptions.backgroundColor = "#00000000";
    winOptions.transparent = true;
    winOptions.vibrancy = "fullscreen-ui";
    winOptions.visualEffectState = "active";
  } else if (isWin) {
    winOptions.frame = false;
    winOptions.backgroundColor = "#1a1a1a";
  } else {
    // Linux
    winOptions.frame = false;
    winOptions.backgroundColor = "#1a1a1a";
  }

  mainWindow = new BrowserWindow(winOptions);

  mainWindow.loadFile(path.join(__dirname, "index.html"));

  mainWindow.once("ready-to-show", () => {
    mainWindow.show();
  });

  mainWindow.on("closed", () => {
    mainWindow = null;
  });

  // Automatically grant media permissions (microphone)
  const session = mainWindow.webContents.session;
  session.setPermissionRequestHandler((webContents, permission, callback) => {
    if (permission === 'media') {
      callback(true);
    } else {
      callback(false);
    }
  });

  session.setPermissionCheckHandler((webContents, permission, requestingOrigin, details) => {
    if (permission === 'media' && details.securityOrigin === 'file:///') {
      return true;
    }
    return false;
  });

  // On macOS, we also need to explicitly ask the OS for permission the first time
  if (process.platform === 'darwin') {
    const { systemPreferences } = require('electron');
    systemPreferences.askForMediaAccess('microphone');
  }
}

// IPC: Get userData path (for onboarding persistence)
ipcMain.handle("get-user-data-path", async () => {
  return app.getPath("userData");
});

// IPC: Window controls (Windows/Linux)
ipcMain.on("window-minimize", () => { if (mainWindow) mainWindow.minimize(); });
ipcMain.on("window-maximize", () => {
  if (mainWindow) {
    mainWindow.isMaximized() ? mainWindow.unmaximize() : mainWindow.maximize();
  }
});
ipcMain.on("window-close", () => { if (mainWindow) mainWindow.close(); });

// IPC: Save API key encrypted with OS-level encryption
ipcMain.handle("save-api-key", async (_event, key) => {
  try {
    if (safeStorage.isEncryptionAvailable()) {
      const encrypted = safeStorage.encryptString(key);
      fs.writeFileSync(keyFilePath, encrypted);
      return { success: true };
    } else {
      // Fallback: save as base64 (not truly encrypted, but better than plaintext)
      fs.writeFileSync(keyFilePath, Buffer.from(key).toString("base64"));
      return { success: true, warning: "OS encryption not available, stored with basic encoding" };
    }
  } catch (err) {
    return { success: false, error: err.message };
  }
});

// IPC: Load API key
ipcMain.handle("load-api-key", async () => {
  try {
    if (!fs.existsSync(keyFilePath)) return { key: null };

    const data = fs.readFileSync(keyFilePath);
    if (safeStorage.isEncryptionAvailable()) {
      const decrypted = safeStorage.decryptString(data);
      return { key: decrypted };
    } else {
      // Fallback: decode base64
      const decoded = Buffer.from(data.toString(), "base64").toString("utf-8");
      return { key: decoded };
    }
  } catch (err) {
    return { key: null, error: err.message };
  }
});

// IPC: Delete stored API key
ipcMain.handle("delete-api-key", async () => {
  try {
    if (fs.existsSync(keyFilePath)) {
      fs.unlinkSync(keyFilePath);
    }
    return { success: true };
  } catch (err) {
    return { success: false, error: err.message };
  }
});

// ── Google OAuth IPC handlers ────────────────────────────────────────────────

// Save Google OAuth token data (access_token + user info JSON string)
ipcMain.handle("google-save-token", async (_event, tokenJson) => {
  try {
    if (safeStorage.isEncryptionAvailable()) {
      const encrypted = safeStorage.encryptString(tokenJson);
      fs.writeFileSync(googleTokenFilePath, encrypted);
    } else {
      fs.writeFileSync(googleTokenFilePath, Buffer.from(tokenJson).toString("base64"));
    }
    return { success: true };
  } catch (err) {
    return { success: false, error: err.message };
  }
});

// Load Google OAuth token data
ipcMain.handle("google-load-token", async () => {
  try {
    if (!fs.existsSync(googleTokenFilePath)) return { token: null };
    const data = fs.readFileSync(googleTokenFilePath);
    let tokenJson;
    if (safeStorage.isEncryptionAvailable()) {
      tokenJson = safeStorage.decryptString(data);
    } else {
      tokenJson = Buffer.from(data.toString(), "base64").toString("utf-8");
    }
    return { token: JSON.parse(tokenJson) };
  } catch (err) {
    return { token: null, error: err.message };
  }
});

// Delete Google OAuth token
ipcMain.handle("google-delete-token", async () => {
  try {
    if (fs.existsSync(googleTokenFilePath)) {
      fs.unlinkSync(googleTokenFilePath);
    }
    return { success: true };
  } catch (err) {
    return { success: false, error: err.message };
  }
});

// Start Google OAuth flow — open browser with authorization URL
// Uses a temporary localhost HTTP server to catch the callback (Desktop app flow)
ipcMain.handle("google-oauth-start", async () => {
  if (!oauthConfig.client_id) {
    return { success: false, error: "OAuth not configured. Add oauth_config.json next to main.js." };
  }
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const urlObj = new URL(req.url, `http://localhost`);
      const code = urlObj.searchParams.get("code");
      const error = urlObj.searchParams.get("error");

      // Send a nice close page back to the browser
      res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      const isSuccess = !!code;
      const logoNoBgB64 = fs.readFileSync(path.join(__dirname, "icons", "AppLogoWithoutBG.png")).toString("base64");
      const logoB64 = fs.readFileSync(path.join(__dirname, "icons", "AppLogo.png")).toString("base64");

      // Load all lang files for client-side i18n
      const langDir = path.join(__dirname, "lang");
      const allTranslations = {};
      fs.readdirSync(langDir).filter(f => f.endsWith(".json")).forEach(f => {
        try {
          const langCode = f.replace(".json", "");
          const data = JSON.parse(fs.readFileSync(path.join(langDir, f), "utf-8"));
          if (data.oauthCallback) allTranslations[langCode] = data.oauthCallback;
        } catch (e) { }
      });
      const translationsJSON = JSON.stringify(allTranslations);
      res.end(`<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>GeminiStudio</title>
  <link rel="icon" type="image/png" href="data:image/png;base64,${logoB64}">
  <style>
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
      background: #111114;
      color: #e8e8ed;
      min-height: 100vh;
      display: flex;
      flex-direction: column;
    }
    header {
      padding: 18px 28px;
      display: flex;
      align-items: center;
      gap: 10px;
      border-bottom: 1px solid rgba(255,255,255,0.07);
      animation: fadeDown 0.4s ease both;
    }
    @keyframes fadeDown {
      from { opacity: 0; transform: translateY(-8px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    .logo-icon {
      width: 28px; height: 28px;
      background: linear-gradient(135deg, #4f8fff, #a78bfa);
      border-radius: 7px;
      display: flex; align-items: center; justify-content: center;
      flex-shrink: 0;
    }
    .logo-name {
      font-size: 15px;
      font-weight: 600;
      letter-spacing: -0.2px;
      color: #e8e8ed;
    }
    main {
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 40px 24px;
      gap: 0;
    }
    .check-row {
      display: flex;
      align-items: center;
      gap: 14px;
      margin-bottom: 16px;
      animation: fadeUp 0.5s 0.1s ease both;
    }
    @keyframes fadeUp {
      from { opacity: 0; transform: translateY(12px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    .check-icon {
      color: ${isSuccess ? "#4f8fff" : "#ff4f4f"};
      flex-shrink: 0;
    }
    h1 {
      font-size: 22px;
      font-weight: 700;
      letter-spacing: -0.4px;
      color: #ffffff;
    }
    .sub1 {
      font-size: 14px;
      color: rgba(232,232,237,0.55);
      margin-bottom: 10px;
      animation: fadeUp 0.5s 0.2s ease both;
    }
    .sub2 {
      font-size: 13px;
      color: rgba(232,232,237,0.35);
      animation: fadeUp 0.5s 0.3s ease both;
    }
    .sub2 a {
      color: #4f8fff;
      text-decoration: none;
    }
    .sub2 a:hover { text-decoration: underline; }
  </style>
</head>
<body>
  <header>
    <img src="data:image/png;base64,${logoNoBgB64}" width="28" height="28" style="border-radius:7px;display:block;" alt="GeminiStudio">
    <span class="logo-name">GeminiStudio</span>
  </header>
  <main>
    <div class="check-row">
      ${isSuccess
          ? `<svg class="check-icon" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`
          : `<svg class="check-icon" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`
        }
      <h1 id="title"></h1>
    </div>
    <p class="sub1" id="sub1"></p>
    <p class="sub2" id="sub2"></p>
  </main>
  <script>
    const isSuccess = ${isSuccess};
    const translations = ${translationsJSON};
    const savedLang = (() => {
      try { return localStorage.getItem("app_language"); } catch(e) { return null; }
    })();
    const browserLang = (savedLang || navigator.language || "en").split("-")[0].toLowerCase();
    const t = translations[browserLang] || translations["en"] || {};

    document.documentElement.lang = browserLang;

    document.getElementById("title").textContent = isSuccess
      ? (t.successTitle || "Login Successful")
      : (t.failureTitle || "Login Failed");

    document.getElementById("sub1").textContent = isSuccess ? (t.closeTitle || "You can close this window") : "";

    const sub2 = document.getElementById("sub2");
    if (isSuccess) {
      sub2.innerHTML = t.returnSuccess || "If GeminiStudio did not launch automatically, <a href='#' onclick='window.close();return false;'>click here</a> to return.";
    } else {
      sub2.textContent = t.returnFailure || "Something went wrong. Please try again.";
    }

    setTimeout(() => window.close(), 3500);
  </script>
</body>
</html>`);

      server.close();
      // Keep oauthCallbackPort set — it will be used in google-oauth-exchange
      // and cleared there after use.

      if (code) {
        sendToRenderer("google-oauth-code", { code });
        if (mainWindow) { mainWindow.focus(); }
      } else {
        sendToRenderer("google-oauth-code", { error: error || "No code received" });
      }
    });

    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      oauthCallbackPort = port;

      const params = new URLSearchParams({
        client_id: oauthConfig.client_id,
        redirect_uri: `http://localhost:${port}`,
        response_type: "code",
        scope: "openid email profile https://www.googleapis.com/auth/generative-language.retriever",
        access_type: "offline",
        prompt: "consent",
      });
      const authUrl = `https://accounts.google.com/o/oauth2/v2/auth?${params.toString()}`;
      shell.openExternal(authUrl);
      resolve({ success: true });
    });

    server.on("error", (err) => {
      resolve({ success: false, error: err.message });
    });
  });
});

// Exchange OAuth code for tokens (called from renderer after receiving callback)
ipcMain.handle("google-oauth-exchange", async (_event, code) => {
  if (!oauthConfig.client_id || !oauthConfig.client_secret) {
    return { success: false, error: "OAuth not configured." };
  }
  // Use the port that was active during google-oauth-start; fallback for safety
  const redirectUri = oauthCallbackPort
    ? `http://localhost:${oauthCallbackPort}`
    : "http://localhost";
  oauthCallbackPort = null; // clear after use
  return new Promise((resolve) => {
    const body = querystring.stringify({
      code,
      client_id: oauthConfig.client_id,
      client_secret: oauthConfig.client_secret,
      redirect_uri: redirectUri,
      grant_type: "authorization_code",
    });
    const req = https.request({
      hostname: "oauth2.googleapis.com",
      path: "/token",
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "Content-Length": Buffer.byteLength(body),
      },
    }, (res) => {
      let data = "";
      res.on("data", (chunk) => { data += chunk; });
      res.on("end", () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.error) {
            resolve({ success: false, error: parsed.error_description || parsed.error });
          } else {
            resolve({ success: true, tokens: parsed });
          }
        } catch (e) {
          resolve({ success: false, error: "Failed to parse token response." });
        }
      });
    });
    req.on("error", (e) => resolve({ success: false, error: e.message }));
    req.write(body);
    req.end();
  });
});

// Refresh Google access token using refresh_token
ipcMain.handle("google-refresh-token", async (_event, refreshToken) => {
  const body = querystring.stringify({
    refresh_token: refreshToken,
    client_id: oauthConfig.client_id,
    client_secret: oauthConfig.client_secret,
    grant_type: "refresh_token",
  });
  return new Promise((resolve) => {
    const req = https.request({
      hostname: "oauth2.googleapis.com",
      path: "/token",
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "Content-Length": Buffer.byteLength(body),
      },
    }, (res) => {
      let data = "";
      res.on("data", (chunk) => { data += chunk; });
      res.on("end", () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.error) {
            resolve({ success: false, error: parsed.error_description || parsed.error });
          } else {
            resolve({ success: true, access_token: parsed.access_token, expires_in: parsed.expires_in || 3600 });
          }
        } catch (e) {
          resolve({ success: false, error: e.message });
        }
      });
    });
    req.on("error", (e) => resolve({ success: false, error: e.message }));
    req.write(body);
    req.end();
  });
});

// Fetch Google user info using access token
ipcMain.handle("google-fetch-userinfo", async (_event, accessToken) => {
  return new Promise((resolve) => {
    const req = https.request({
      hostname: "www.googleapis.com",
      path: "/oauth2/v3/userinfo",
      method: "GET",
      headers: { Authorization: `Bearer ${accessToken}` },
    }, (res) => {
      let data = "";
      res.on("data", (chunk) => { data += chunk; });
      res.on("end", () => {
        try {
          resolve({ success: true, user: JSON.parse(data) });
        } catch {
          resolve({ success: false });
        }
      });
    });
    req.on("error", () => resolve({ success: false }));
    req.end();
  });
});

// ── Themes Import/Export ─────────────────────────────────────────────────────

ipcMain.handle("theme:export", async () => {
  try {
    const defaultDarkPath = path.join(__dirname, "themes", "dark.css");
    if (!fs.existsSync(defaultDarkPath)) throw new Error("dark.css not found.");

    const { canceled, filePath } = await dialog.showSaveDialog(mainWindow, {
      title: "Export Theme",
      defaultPath: "dark.css",
      filters: [{ name: "CSS Files", extensions: ["css"] }]
    });

    if (canceled || !filePath) return { success: false, canceled: true };

    fs.copyFileSync(defaultDarkPath, filePath);
    return { success: true, filePath };
  } catch (err) {
    return { success: false, error: err.message };
  }
});

ipcMain.handle("theme:import", async () => {
  try {
    const { canceled, filePaths } = await dialog.showOpenDialog(mainWindow, {
      title: "Import Theme",
      filters: [{ name: "CSS Files", extensions: ["css"] }],
      properties: ["openFile"]
    });

    if (canceled || filePaths.length === 0) return { success: false, canceled: true };

    const sourcePath = filePaths[0];
    const fileName = path.basename(sourcePath);

    // Safety check
    if (fileName === "dark.css" || fileName === "light.css") {
      return { success: false, error: "Cannot overwrite core theme files." };
    }

    const userThemesDir = path.join(__dirname, "user-themes");
    if (!fs.existsSync(userThemesDir)) fs.mkdirSync(userThemesDir, { recursive: true });

    const destPath = path.join(userThemesDir, fileName);
    fs.copyFileSync(sourcePath, destPath);

    return { success: true, fileName };
  } catch (err) {
    return { success: false, error: err.message };
  }
});

ipcMain.handle("theme:get-custom", async () => {
  try {
    const userThemesDir = path.join(__dirname, "user-themes");
    if (!fs.existsSync(userThemesDir)) return { themes: [] };

    const files = fs.readdirSync(userThemesDir).filter(f => f.endsWith(".css"));

    return { themes: files };
  } catch (err) {
    return { themes: [] };
  }
});

ipcMain.handle("theme:delete", async (event, fileName) => {
  try {
    const userThemesDir = path.join(__dirname, "user-themes");
    const filePath = path.join(userThemesDir, fileName);
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
      return { success: true };
    }
    return { success: false, error: "File not found" };
  } catch (err) {
    return { success: false, error: err.message };
  }
});

// Focus window if a second instance is launched
app.on("second-instance", () => {
  if (mainWindow) {
    if (mainWindow.isMinimized()) mainWindow.restore();
    mainWindow.focus();
  }
});

app.whenReady().then(createWindow);

app.on("window-all-closed", () => {
  app.quit();
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
