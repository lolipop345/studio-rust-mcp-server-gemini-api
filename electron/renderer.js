const { Marked } = require("marked");
const { markedHighlight } = require("marked-highlight");
const hljs = require("highlight.js");
const { shell, ipcRenderer } = require("electron");
const os = require("os");
const path = require("path");
const { exec } = require("child_process");
const fs = require("fs");

window.addEventListener('error', (e) => {
    try { fs.appendFileSync('/tmp/renderer.log', (e.error ? e.error.stack : e.message) + '\n'); } catch (err) { }
});
window.addEventListener('unhandledRejection', (e) => {
    try { fs.appendFileSync('/tmp/renderer.log', (e.reason ? (e.reason.stack || e.reason) : 'Promise rejection') + '\n'); } catch (err) { }
});

const API_BASE = "http://127.0.0.1:44755";

// ─── i18n System ───────────────────────────────────────────────
const SUPPORTED_LANGS = ["en", "tr", "ru", "pt", "de", "nl", "fr", "it", "es", "pl", "bg", "sr", "be"];
let currentLang = {};
let currentLangCode = "en";

function loadLanguageSync(code) {
    const langPath = path.join(__dirname, "lang", `${code}.json`);
    try {
        const raw = fs.readFileSync(langPath, "utf-8");
        currentLang = JSON.parse(raw);
        currentLangCode = code;
    } catch (err) {
        console.error(`Failed to load language ${code}:`, err);
        // Fallback to English
        if (code !== "en") loadLanguageSync("en");
    }
}

// Resolve dotted key like "settings.title"
function t(key, replacements) {
    const keys = key.split(".");
    let val = currentLang;
    for (const k of keys) {
        if (val && typeof val === "object" && k in val) {
            val = val[k];
        } else {
            return key; // fallback: return key itself
        }
    }
    if (typeof val === "string" && replacements) {
        for (const [rk, rv] of Object.entries(replacements)) {
            // Match both {key} and {{key}}
            val = val.replace(new RegExp(`\\{\\{?${rk}\\}\\}?`, 'g'), rv);
        }
    }
    return val;
}

let cachedFullName = null;
function getSystemFullName() {
    if (cachedFullName) return cachedFullName;
    try {
        cachedFullName = os.userInfo().username;
    } catch (e) {
        cachedFullName = os.hostname().split(".")[0] || "User";
    }

    if (cachedFullName) {
        cachedFullName = cachedFullName.charAt(0).toUpperCase() + cachedFullName.slice(1);
    }
    return cachedFullName || "User";
}

// Apply translations to all elements with data-i18n attribute
function applyTranslations() {
    document.querySelectorAll("[data-i18n]").forEach((el) => {
        const key = el.getAttribute("data-i18n");
        const translated = t(key);
        if (typeof translated === "string") {
            el.textContent = translated;
        }
    });

    // Apply placeholder translations
    document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
        const key = el.getAttribute("data-i18n-placeholder");
        const translated = t(key);
        if (typeof translated === "string") {
            el.placeholder = translated;
        }
    });

    // Apply title translations
    document.querySelectorAll("[data-i18n-title]").forEach((el) => {
        const key = el.getAttribute("data-i18n-title");
        const translated = t(key);
        if (typeof translated === "string") {
            el.title = translated;
        }
    });

    const motivationalPhrases = t("landing.motivationalPhrases");
    const headerTitle = document.querySelector(".hero-title");
    if (headerTitle && Array.isArray(motivationalPhrases) && motivationalPhrases.length > 0) {
        if (!headerTitle.hasAttribute('data-set-lang') || headerTitle.getAttribute('data-set-lang') !== currentLangCode) {
            const randomPhrase = motivationalPhrases[Math.floor(Math.random() * motivationalPhrases.length)];
            headerTitle.textContent = randomPhrase;
            headerTitle.setAttribute('data-set-lang', currentLangCode);
        }
    }

    const greetingEl = document.getElementById("hero-greeting");
    if (greetingEl) {
        try {
            const name = getSystemFullName();
            greetingEl.textContent = t("landing.hello", { name });
        } catch (e) {
            greetingEl.textContent = t("landing.hello", { name: os.hostname().split(".")[0] || "User" });
        }
    }
}

function detectOSLanguage() {
    const locale = Intl.DateTimeFormat().resolvedOptions().locale || navigator.language || "en";
    const langCode = locale.split("-")[0].toLowerCase();
    return SUPPORTED_LANGS.includes(langCode) ? langCode : "en";
}

function initLanguage() {
    const saved = localStorage.getItem("app_language");
    const code = saved || detectOSLanguage();
    loadLanguageSync(code);
    localStorage.setItem("app_language", code);
    applyTranslations();
    // Set dropdown value
    const langSelect = document.getElementById("language-select");
    if (langSelect) langSelect.value = code;
}

const marked = new Marked(
    markedHighlight({
        langPrefix: "hljs language-",
        highlight(code, lang) {
            if (lang && hljs.getLanguage(lang)) {
                return hljs.highlight(code, { language: lang }).value;
            }
            return hljs.highlightAuto(code).value;
        },
    })
);

marked.setOptions({
    breaks: true,
    gfm: true,
});

const messagesEl = document.getElementById("messages");
const welcomeEl = document.getElementById("welcome");
const inputEl = document.getElementById("message-input");
const sendBtn = document.getElementById("btn-send");
const themeBtn = document.getElementById("btn-theme");
const settingsBtn = document.getElementById("btn-settings");
const ghostBtn = document.getElementById("btn-ghost");
const modelBtn = document.getElementById("btn-model-select");
const modelPopover = document.getElementById("model-popover");
const moreModelsBtn = document.getElementById("more-models-btn");
const moreModelsSection = document.getElementById("more-models-section");
const modalOverlay = document.getElementById("modal-overlay");
const apiKeyInput = document.getElementById("api-key-input");
const saveKeyBtn = document.getElementById("btn-save-key");
const toggleKeyBtn = document.getElementById("btn-toggle-key");
const statusDot = document.getElementById("status-dot");
const statusText = document.getElementById("status-text");
const iconMoon = document.getElementById("icon-moon");
const iconSun = document.getElementById("icon-sun");
const getKeyLink = document.getElementById("get-key-link");
const hljsThemeLink = document.getElementById("hljs-theme");

// New UI Elements
const btnAttach = document.getElementById("btn-attach");
const artifactSidebar = document.getElementById("artifact-sidebar");
const artifactContentArea = document.getElementById("artifact-content-area");
const btnArtifactProceed = document.getElementById("btn-artifact-proceed");

const btnSearch = document.getElementById("btn-search");
const btnCode = document.getElementById("btn-code");
const btnSidebarToggle = document.getElementById("btn-sidebar-toggle");
const btnNewChat = document.getElementById("btn-new-chat");
const sidebar = document.getElementById("sidebar");
const btnClearHistory = document.getElementById("btn-clear-history");
const btnResetCookies = document.getElementById("btn-reset-cookies");
const btnModalClose = document.getElementById("btn-modal-close");
const btnGoogleSignin = document.getElementById("btn-google-signin");
const googleAccountCard = document.getElementById("google-account-card");
const googleAvatar = document.getElementById("google-avatar");
const googleEmailEl = document.getElementById("google-email");
const btnGoogleLogout = document.getElementById("btn-google-logout");
const imageUpload = document.getElementById("image-upload");
const imagePreview = document.getElementById("image-preview");
const previewImg = document.getElementById("preview-img");
const btnRemoveImg = document.getElementById("btn-remove-img");

const systemPromptInput = document.getElementById("system-prompt-input");
const tempSlider = document.getElementById("temp-slider");
const tempVal = document.getElementById("temp-val");

const btnOpenSearch = document.getElementById("btn-open-search");
const searchView = document.getElementById("search-view");
const btnCloseSearch = document.getElementById("btn-close-search");
const activeSearchInput = document.getElementById("active-search-input");
const searchViewResults = document.getElementById("search-view-results");
let searchDebounce = null;

const confirmModalOverlay = document.getElementById("confirm-modal-overlay");
const confirmModalTitle = document.getElementById("confirm-modal-title");
const confirmModalMessage = document.getElementById("confirm-modal-message");
const btnConfirmCancel = document.getElementById("btn-confirm-cancel");
const btnConfirmOk = document.getElementById("btn-confirm-ok");

// State Variables
let isGoogleLoggedIn = false;
let googleUserInfo = null; // { email, picture, access_token }
let isProcessing = false;
let currentModel = "";
let currentThinkingLevel = "none"; // Disabled by default until toggle is checked
let useSearch = true; // Web Grounding default ON
let useCode = false;
let attachedFiles = []; // Array of {base64, mimeType, name, dataUrl}
let isExplainingError = false; // Guard flag to prevent recursive error loops
let userAvatarBase64 = null;
let activeEventSource = null;
let currentChatId = null;
let isAborting = false;
let currentArtifactComments = {}; // block_index -> comment string
let activeArtifactTx = null; // store the chat ID waiting for the plan

// ─── Processing State & Abort ──────────────────────────────────────
function setProcessingState(processing) {
    isProcessing = processing;
    if (processing) {
        sendBtn.classList.add("active", "stop-btn");
        sendBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>`;
    } else {
        sendBtn.classList.remove("stop-btn");
        updateSendButtonState();
        sendBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 19V5M5 12l7-7 7 7"/></svg>`;
    }
}

function updateSendButtonState() {
    if ((inputEl.value.trim() || attachedFiles.length > 0) && !isProcessing) {
        sendBtn.classList.add("active");
    } else {
        sendBtn.classList.remove("active");
    }
}

let finishGenerationHandler = null;

function abortGeneration() {
    isAborting = true;
    if (typeof closeArtifactSidebar === "function") {
        closeArtifactSidebar();
    }
    if (activeEventSource) {

        // Immediately close the network connection
        activeEventSource.close();
        activeEventSource = null;

        // Cleanup UI: Remove "Thinking" or "Connecting" indicators immediately
        const typingMsg = document.getElementById("typing-message");
        if (typingMsg) {
            const indicators = typingMsg.querySelectorAll(".thinking-indicator, .connecting-indicator");
            indicators.forEach(el => el.remove());

            // Also remove thought chains and tool calls during abort as per user requirement
            const temporaryStuff = typingMsg.querySelectorAll(".thought-chain, .tool-call");
            temporaryStuff.forEach(el => el.remove());

            // If the message bubble is completely empty after cleaning, remove the whole container
            const pContent = typingMsg.querySelector('.message-content');
            if (pContent && (!pContent.innerHTML || pContent.innerHTML.trim() === '')) {
                typingMsg.remove();
            }
        }

        // Send stop request to backend (background)
        if (currentChatId) {
            fetch(`${API_BASE}/chat/stop`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: currentChatId })
            }).catch(e => console.error("Abort failed", e));
        }

        // Finalize the UI state
        if (finishGenerationHandler) {
            finishGenerationHandler();
        }
    }
}

// ─── MIME Type Helper ──────────────────────────────────────────────
// MIME types Gemini API natively accepts (inline data)
const GEMINI_NATIVE_MIMES = new Set([
    'image/png', 'image/jpeg', 'image/webp', 'image/heic', 'image/heif', 'image/bmp',
    'audio/wav', 'audio/mp3', 'audio/mpeg', 'audio/aiff', 'audio/aac', 'audio/ogg',
    'audio/flac', 'audio/mp4', 'audio/webm',
    'video/mp4', 'video/mpeg', 'video/quicktime', 'video/avi', 'video/x-msvideo',
    'video/x-flv', 'video/mpg', 'video/webm', 'video/wmv', 'video/3gpp', 'video/x-matroska',
    'application/pdf', 'application/json',
    'text/plain', 'text/html', 'text/css', 'text/javascript', 'text/typescript',
    'text/csv', 'text/markdown', 'text/xml', 'text/rtf',
]);

// Extension → raw MIME lookup
const EXTENSION_MIME_MAP = {
    'png': 'image/png', 'jpg': 'image/jpeg', 'jpeg': 'image/jpeg',
    'webp': 'image/webp', 'bmp': 'image/bmp', 'heic': 'image/heic', 'heif': 'image/heif',
    // Not natively supported by Gemini — will be converted to text/plain:
    'svg': 'image/svg+xml', 'gif': 'image/gif', 'ico': 'image/x-icon',
    'tiff': 'image/tiff', 'tif': 'image/tiff', 'avif': 'image/avif',
    'mp3': 'audio/mpeg', 'wav': 'audio/wav', 'ogg': 'audio/ogg',
    'flac': 'audio/flac', 'aac': 'audio/aac', 'weba': 'audio/webm', 'm4a': 'audio/mp4',
    'mp4': 'video/mp4', 'webm': 'video/webm', 'mov': 'video/quicktime',
    'avi': 'video/x-msvideo', 'mkv': 'video/x-matroska',
    'pdf': 'application/pdf', 'json': 'application/json',
    'txt': 'text/plain', 'csv': 'text/csv', 'xml': 'text/xml',
    'html': 'text/html', 'htm': 'text/html', 'css': 'text/css',
    'js': 'text/javascript', 'ts': 'text/typescript', 'jsx': 'text/javascript', 'tsx': 'text/typescript',
    'md': 'text/markdown', 'markdown': 'text/markdown', 'rtf': 'text/rtf',
    // Code files → text/plain
    'py': 'text/plain', 'lua': 'text/plain', 'rb': 'text/plain', 'rs': 'text/plain',
    'go': 'text/plain', 'java': 'text/plain', 'c': 'text/plain', 'cpp': 'text/plain',
    'cs': 'text/plain', 'sh': 'text/plain', 'bash': 'text/plain', 'zsh': 'text/plain',
    'yaml': 'text/plain', 'yml': 'text/plain', 'toml': 'text/plain',
    'ini': 'text/plain', 'cfg': 'text/plain', 'conf': 'text/plain', 'log': 'text/plain',
    // Roblox Studio files (XML-based) → text/xml
    'rbxlx': 'text/xml', 'rbxm': 'text/xml', 'rbxmx': 'text/xml',
};

function getMimeType(fileName, detectedMime, fileType) {
    // 1. Try file.type from the File API (most reliable when available)
    if (fileType && fileType !== 'application/octet-stream' && fileType !== '') {
        return fileType;
    }
    // 2. Try the MIME from the data URL
    if (detectedMime && detectedMime !== 'application/octet-stream') {
        return detectedMime;
    }
    // 3. Fallback to extension-based lookup
    const ext = (fileName || '').split('.').pop().toLowerCase();
    return EXTENSION_MIME_MAP[ext] || detectedMime || 'application/octet-stream';
}

// Detect MIME from base64 magic bytes
function detectMimeFromBase64(base64) {
    if (!base64 || base64.length < 8) return null;
    const header = base64.substring(0, 16);
    if (header.startsWith('iVBOR')) return 'image/png';
    if (header.startsWith('/9j/')) return 'image/jpeg';
    if (header.startsWith('UklGR')) return 'image/webp';
    if (header.startsWith('Qk')) return 'image/bmp';
    if (header.startsWith('AAAA')) return 'video/mp4';
    if (header.startsWith('JVBERi')) return 'application/pdf';
    if (header.startsWith('T2dnU')) return 'audio/ogg';
    if (header.startsWith('R0lGO')) return 'image/gif'; // GIF — not natively supported, will become text
    return null;
}

// Convert any MIME to one Gemini can handle.
// If natively supported → keep. If text-readable → text/plain. Otherwise → null (reject).
function toGeminiMime(mimeType) {
    if (!mimeType) return null;
    if (GEMINI_NATIVE_MIMES.has(mimeType)) return mimeType;
    if (mimeType.startsWith('text/')) return 'text/plain';
    if (mimeType.includes('xml') || mimeType.includes('xhtml') || mimeType.includes('svg')) return 'text/plain';
    if (mimeType.includes('json')) return 'text/plain';
    return null; // Binary / unsupported
}

// Check if a base64-encoded file is likely binary (has null bytes in the first 512 bytes)
function isLikelyBinary(base64) {
    const binary = atob(base64.substring(0, 700));
    for (let i = 0; i < binary.length; i++) {
        if (binary.charCodeAt(i) === 0) return true;
    }
    return false;
}

async function loadUserAvatar() {
    return new Promise((resolve) => {
        if (os.platform() === 'darwin') {
            const username = os.userInfo().username;
            exec(`dscl . -read /Users/${username} JPEGPhoto | tail -n +2 | xxd -r -p | base64`, (err, stdout) => {
                const b64 = stdout ? stdout.trim() : "";
                if (!err && b64.length > 100) {
                    userAvatarBase64 = `data:image/jpeg;base64,${b64}`;
                }
                resolve();
            });
        } else if (os.platform() === 'win32') {
            // Windows 10/11 typically stores the current user's picture path in this registry key
            const script = `
            $path = (Get-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\AccountPicture' -Name 'SourceId' -ErrorAction SilentlyContinue).SourceId;
            if ($path -and (Test-Path $path)) {
                [Convert]::ToBase64String([IO.File]::ReadAllBytes($path))
            }
            `;
            exec(`powershell -NoProfile -Command "${script.replace(/\n/g, '')}"`, (err, stdout) => {
                const b64 = stdout ? stdout.trim() : "";
                if (!err && b64.length > 100) {
                    // Windows Account pictures can be JPEG or PNG. Data URI jpeg works nicely for both if base64 content is valid, 
                    // or we check the magic bytes. We'll default to jpeg or png based on header, or just use image/jpeg as a fallback.
                    const mime = b64.startsWith("/9j/") ? "image/jpeg" : "image/png";
                    userAvatarBase64 = `data:${mime};base64,${b64}`;
                }
                resolve();
            });
        } else {
            resolve();
        }
    });
}

function initTheme() {
    const saved = localStorage.getItem("theme");
    let theme;
    if (saved) {
        theme = saved;
    } else {
        // Detect OS color scheme (works on macOS & Windows)
        const prefersDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
        theme = prefersDark ? "dark" : "light";
        localStorage.setItem("theme", theme);
    }
    document.documentElement.setAttribute("data-theme", theme);
    updateThemeIcons(theme);
    updateHljsTheme(theme);

    // Listen for OS theme changes in real-time
    if (window.matchMedia) {
        window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
            const newTheme = e.matches ? "dark" : "light";
            document.documentElement.setAttribute("data-theme", newTheme);
            localStorage.setItem("theme", newTheme);
            updateThemeIcons(newTheme);
            updateHljsTheme(newTheme);
            updateThemeIcons(newTheme);
            updateHljsTheme(newTheme);
        });
    }
}

function toggleTheme() {
    const current = document.documentElement.getAttribute("data-theme") || "dark";
    const next = current === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", next);
    localStorage.setItem("theme", next);
    updateThemeIcons(next);
    updateHljsTheme(next);
}

function updateThemeIcons(theme) {
    if (theme === "dark") {
        iconMoon.style.display = "block";
        iconSun.style.display = "none";
    } else {
        iconMoon.style.display = "none";
        iconSun.style.display = "block";
    }
}

function updateHljsTheme(theme) {
    if (theme === "dark") {
        hljsThemeLink.href = "node_modules/highlight.js/styles/github-dark.min.css";
    } else {
        hljsThemeLink.href = "node_modules/highlight.js/styles/github.min.css";
    }
}

async function checkStatus() {
    try {
        const res = await fetch(`${API_BASE}/chat/status`);
        const data = await res.json();
        if (data.api_key_set) {
            statusDot.className = "connected";
            statusText.textContent = t("status.connected");
            return true;
        } else {
            statusDot.className = "disconnected";
            statusText.textContent = t("status.apiKeyNotSet");
            return false;
        }
    } catch {
        statusDot.className = "disconnected";
        statusText.textContent = t("status.serverNotRunning");
        return false;
    }
}

async function setApiKey(key) {
    const res = await fetch(`${API_BASE}/chat/api-key`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ api_key: key }),
    });
    return res.ok;
}

let savedApiKeyLoaded = false; // Track if the displayed key is the masked saved one

async function showModal() {
    modalOverlay.classList.remove("hidden");

    // Always reset to first tab
    document.querySelectorAll(".tab-btn").forEach(b => b.classList.remove("active"));
    document.querySelectorAll(".tab-panel").forEach(p => p.classList.add("hidden"));
    const firstTab = document.querySelector(".tab-btn[data-tab='tab-api']");
    if (firstTab) firstTab.classList.add("active");
    const firstPanel = document.getElementById("tab-api");
    if (firstPanel) firstPanel.classList.remove("hidden");

    // If we have a saved key, show masked version
    try {
        const result = await ipcRenderer.invoke("load-api-key");
        if (result.key) {
            const key = result.key;
            // Mask: show first 4 and last 4, rest as dots
            const masked = key.length > 8
                ? key.substring(0, 4) + "•".repeat(key.length - 8) + key.substring(key.length - 4)
                : "•".repeat(key.length);
            apiKeyInput.value = masked;
            apiKeyInput.type = "password";
            savedApiKeyLoaded = true;
        } else {
            apiKeyInput.value = "";
            savedApiKeyLoaded = false;
        }
    } catch {
        savedApiKeyLoaded = false;
    }

    if (!isGoogleLoggedIn) apiKeyInput.focus();
}

// When user starts typing, clear the masked key
apiKeyInput.addEventListener("focus", () => {
    if (savedApiKeyLoaded) {
        apiKeyInput.value = "";
        apiKeyInput.placeholder = t("settings.enterNewKey");
        savedApiKeyLoaded = false;
    }
});

// Prevent copying the masked value
apiKeyInput.addEventListener("copy", (e) => {
    e.preventDefault();
});

function hideModal() {
    modalOverlay.classList.add("hidden");
}

// ── Settings tab switching ──────────────────────────────────────────────────
document.querySelectorAll(".tab-btn").forEach(btn => {
    btn.addEventListener("click", () => {
        document.querySelectorAll(".tab-btn").forEach(b => b.classList.remove("active"));
        document.querySelectorAll(".tab-panel").forEach(p => p.classList.add("hidden"));
        btn.classList.add("active");
        const panel = document.getElementById(btn.dataset.tab);
        if (panel) panel.classList.remove("hidden");
    });
});

// Close modal via X button
if (btnModalClose) {
    btnModalClose.addEventListener("click", hideModal);
}

// ── Google OAuth ────────────────────────────────────────────────────────────
function updateGoogleLoginUI() {
    if (isGoogleLoggedIn && googleUserInfo) {
        googleAccountCard.classList.remove("hidden");
        btnGoogleSignin.classList.add("hidden");
        googleEmailEl.textContent = googleUserInfo.email || "";
        if (googleUserInfo.picture) {
            googleAvatar.src = googleUserInfo.picture;
        }
        // Disable API key input
        const keyWrapper = document.getElementById("key-input-wrapper");
        if (keyWrapper) keyWrapper.classList.add("input-disabled");
    } else {
        googleAccountCard.classList.add("hidden");
        btnGoogleSignin.classList.remove("hidden");
        const keyWrapper = document.getElementById("key-input-wrapper");
        if (keyWrapper) keyWrapper.classList.remove("input-disabled");
    }
}

// Set Google OAuth access token on the backend
async function setGoogleToken(accessToken) {
    try {
        const res = await fetch(`${API_BASE}/chat/api-key`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ api_key: `Bearer ${accessToken}` }),
        });
        return res.ok;
    } catch {
        return false;
    }
}

if (btnGoogleSignin) {
    btnGoogleSignin.addEventListener("click", async () => {
        const result = await ipcRenderer.invoke("google-oauth-start");
        if (!result.success) {
            console.error("OAuth start failed:", result.error);
            // Show error in UI
            const existing = document.querySelector("#tab-api .error-text");
            if (existing) existing.remove();
            const err = document.createElement("p");
            err.className = "error-text";
            err.textContent = result.error || t("settings.oauthError", { default: "OAuth not configured. Add oauth_config.json." });
            document.getElementById("tab-api").appendChild(err);
        }
    });
}

// Listen for OAuth callback from main process
ipcRenderer.on("google-oauth-code", async (_event, data) => {
    if (data.error) {
        console.error("OAuth error:", data.error);
        return;
    }
    // Exchange code for tokens
    const tokenResult = await ipcRenderer.invoke("google-oauth-exchange", data.code);
    if (!tokenResult.success) {
        console.error("Token exchange failed:", tokenResult.error);
        return;
    }
    const tokens = tokenResult.tokens;
    // Fetch user info
    const userResult = await ipcRenderer.invoke("google-fetch-userinfo", tokens.access_token);
    const user = userResult.success ? userResult.user : {};
    // Store token + user info
    const tokenData = {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token || null,
        expires_at: Date.now() + ((tokens.expires_in || 3600) * 1000),
        email: user.email || "",
        picture: user.picture || "",
        name: user.name || user.given_name || "",
    };
    await ipcRenderer.invoke("google-save-token", JSON.stringify(tokenData));
    // Update state
    googleUserInfo = tokenData;
    isGoogleLoggedIn = true;
    // Send token to backend
    await setGoogleToken(tokens.access_token);
    await checkStatus();
    await fetchModels();
    updateGoogleLoginUI();
    // Focus window
    mainWindow && mainWindow.focus && mainWindow.focus();
});

if (btnGoogleLogout) {
    btnGoogleLogout.addEventListener("click", async () => {
        await ipcRenderer.invoke("google-delete-token");
        isGoogleLoggedIn = false;
        googleUserInfo = null;
        // Re-load saved API key if available
        try {
            const result = await ipcRenderer.invoke("load-api-key");
            if (result.key) {
                await setApiKey(result.key);
            } else {
                await ipcRenderer.invoke("delete-api-key");
            }
        } catch {}
        await checkStatus();
        updateGoogleLoginUI();
    });
}

// Restore Google login on startup
async function restoreGoogleLogin() {
    try {
        const result = await ipcRenderer.invoke("google-load-token");
        if (!result.token || !result.token.access_token) return;

        let token = result.token;

        // Check if access token is expired (5 min buffer)
        const isExpired = token.expires_at && Date.now() > token.expires_at - 300000;
        if (isExpired) {
            if (!token.refresh_token) {
                // No refresh token — force re-login
                await ipcRenderer.invoke("google-delete-token");
                return;
            }
            const refreshResult = await ipcRenderer.invoke("google-refresh-token", token.refresh_token);
            if (!refreshResult.success) {
                // Refresh failed — force re-login
                await ipcRenderer.invoke("google-delete-token");
                return;
            }
            token = {
                ...token,
                access_token: refreshResult.access_token,
                expires_at: Date.now() + (refreshResult.expires_in * 1000),
            };
            await ipcRenderer.invoke("google-save-token", JSON.stringify(token));
        }

        // If name is missing (old session), try to fetch it
        if (!token.name && token.access_token) {
            try {
                const userResult = await ipcRenderer.invoke("google-fetch-userinfo", token.access_token);
                if (userResult.success && userResult.user) {
                    token.name = userResult.user.name || userResult.user.given_name || "";
                    token.picture = token.picture || userResult.user.picture || "";
                    await ipcRenderer.invoke("google-save-token", JSON.stringify(token));
                }
            } catch {}
        }
        googleUserInfo = token;
        isGoogleLoggedIn = true;
        await setGoogleToken(token.access_token);
        updateGoogleLoginUI();
    } catch {}
}

// ── Cookie / Preferences Reset ─────────────────────────────────────────────
if (btnResetCookies) {
    btnResetCookies.addEventListener("click", async () => {
        const confirmed = await showConfirm(
            t("settings.resetCookies", { default: "Reset Preferences" }),
            t("settings.resetCookiesConfirm", { default: "Theme, language, and other local preferences will be reset to defaults. The app will reload." })
        );
        if (confirmed) {
            localStorage.removeItem("theme");
            localStorage.removeItem("app_language");
            location.reload();
        }
    });
}

function showConfirm(title, message) {
    return new Promise((resolve) => {
        confirmModalTitle.textContent = title;
        confirmModalMessage.textContent = message;
        confirmModalOverlay.classList.remove("hidden");

        const handleCancel = () => {
            cleanup();
            resolve(false);
        };

        const handleOk = () => {
            cleanup();
            resolve(true);
        };

        const cleanup = () => {
            confirmModalOverlay.classList.add("hidden");
            btnConfirmCancel.removeEventListener("click", handleCancel);
            btnConfirmOk.removeEventListener("click", handleOk);
        };

        btnConfirmCancel.addEventListener("click", handleCancel);
        btnConfirmOk.addEventListener("click", handleOk);
    });
}

// ─── Chat Tree State ───────────────────────────────────────────────
let chatTree = {};
let activeNodeId = null;
let currentSessionId = null;
let currentSessionTitle = "New Chat";
let isSessionPinned = false;

function createNode(role, content, parentId = null, extraNodesHTML = "", images = [], info = null, thoughtSignature = null) {
    const id = "node_" + Date.now() + "_" + Math.random().toString(36).substr(2, 9);
    const node = {
        id,
        parentId,
        role,
        content,
        extraNodesHTML,
        images,
        children: [],
        timestamp: new Date().toISOString(),
        duration: 0,
        info,
        thoughtSignature
    };
    chatTree[id] = node;
    chatTree[id] = node;
    if (parentId && chatTree[parentId]) {
        chatTree[parentId].children.push(id);
    }
    return node;
}

function generateUUID() {
    if (typeof crypto !== 'undefined' && crypto.randomUUID) {
        return crypto.randomUUID();
    }
    return "10000000-1000-4000-8000-100000000000".replace(/[018]/g, c =>
        (c ^ crypto.getRandomValues(new Uint8Array(1))[0] & 15 >> c / 4).toString(16)
    );
}

// ─── History API ──────────────────────────────────────────────────

function updateTopbar(liveTokens = 0) {
    const titleEl = document.getElementById("topbar-chat-title");
    const tokensEl = document.getElementById("topbar-chat-tokens");
    if (!titleEl || !tokensEl) return;

    titleEl.textContent = currentSessionTitle || "New Chat";

    let totalSessionTokens = 0;
    if (activeNodeId) {
        const path = getBranchPath(activeNodeId);
        path.forEach(node => {
            if (node.info && node.info.tokens) {
                totalSessionTokens += parseInt(node.info.tokens) || 0;
            }
        });
    }

    totalSessionTokens += liveTokens;

    if (totalSessionTokens > 0) {
        tokensEl.innerHTML = `<span style="margin: 0 6px; opacity: 0.3;">|</span><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="margin-right: 4px;"><polygon points="12 2 2 7 12 12 22 7 12 2"></polygon><polyline points="2 17 12 22 22 17"></polyline><polyline points="2 12 12 17 22 12"></polyline></svg>${totalSessionTokens.toLocaleString()} tokens`;
    } else {
        tokensEl.innerHTML = "";
    }
}

async function loadSidebarHistory() {
    try {
        const res = await fetch(`${API_BASE}/history/list`);
        if (!res.ok) return;
        const chats = await res.json();

        const pinnedList = document.getElementById("sidebar-pinned-list");
        const recentList = document.getElementById("sidebar-recent-list");
        const sectionPinned = document.getElementById("section-pinned");

        pinnedList.innerHTML = "";
        recentList.innerHTML = "";

        let hasPinned = false;

        chats.forEach(chat => {
            const item = document.createElement("div");
            item.className = "sidebar-item";
            if (chat.id === currentSessionId) item.classList.add("active");

            const titleEl = document.createElement("div");
            titleEl.className = "sidebar-item-title";
            titleEl.textContent = chat.title || "New Chat";
            titleEl.title = chat.title || "New Chat";

            // Double-click to rename
            titleEl.addEventListener("dblclick", () => {
                const input = document.createElement("input");
                input.className = "sidebar-item-title-input";
                input.value = chat.title;
                item.replaceChild(input, titleEl);
                input.focus();

                let isSaving = false;
                const saveTitle = async () => {
                    if (isSaving) return;
                    isSaving = true;
                    const newTitle = input.value.trim() || "New Chat";
                    await fetch(`${API_BASE}/history/update_title`, {
                        method: "POST",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify({ id: chat.id, title: newTitle })
                    });
                    if (chat.id === currentSessionId) {
                        currentSessionTitle = newTitle;
                        updateTopbar();
                    }
                    loadSidebarHistory();
                };

                input.addEventListener("blur", saveTitle);
                input.addEventListener("keydown", (e) => {
                    if (e.key === "Enter") saveTitle();
                    if (e.key === "Escape") loadSidebarHistory();
                });
            });

            const actions = document.createElement("div");
            actions.className = "sidebar-item-actions dropdown-container";

            const ellipsisBtn = document.createElement("button");
            ellipsisBtn.className = "sidebar-btn";
            ellipsisBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="1"></circle><circle cx="12" cy="5" r="1"></circle><circle cx="12" cy="19" r="1"></circle></svg>`;

            const menu = document.createElement("div");
            menu.className = "sidebar-item-menu hidden";

            const renameBtn = document.createElement("button");
            renameBtn.className = "sidebar-btn";
            renameBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path></svg> Rename`;
            renameBtn.onclick = (e) => {
                e.stopPropagation();
                menu.classList.add("hidden");
                titleEl.dispatchEvent(new MouseEvent("dblclick"));
            };

            const pinBtn = document.createElement("button");
            pinBtn.className = `sidebar-btn btn-pin ${chat.pinned ? 'pinned' : ''}`;
            pinBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="${chat.pinned ? 'currentColor' : 'none'}" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22"></line><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.68V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3v4.68a2 2 0 0 1-1.11 1.87l-1.78.9A2 2 0 0 0 5 15.24Z"></path></svg>`;
            pinBtn.appendChild(document.createTextNode(chat.pinned ? " Unpin" : " Pin"));
            pinBtn.onclick = async (e) => {
                e.stopPropagation();
                await fetch(`${API_BASE}/history/toggle_pin`, {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ id: chat.id, pinned: !chat.pinned })
                });
                if (chat.id === currentSessionId) isSessionPinned = !chat.pinned;
                loadSidebarHistory();
            };

            const delBtn = document.createElement("button");
            delBtn.className = "sidebar-btn btn-delete";
            delBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg> Delete`;
            delBtn.onclick = async (e) => {
                e.stopPropagation();
                if (confirm(t("sidebar.confirmDelete", { default: "Are you sure you want to delete this chat?" }))) {
                    await fetch(`${API_BASE}/history/delete/${chat.id}`, { method: "POST" });
                    if (chat.id === currentSessionId) {
                        startNewSession(); // switch to blank session
                    } else {
                        loadSidebarHistory();
                    }
                }
            };

            item.onclick = () => loadSession(chat.id);

            menu.appendChild(renameBtn);
            menu.appendChild(pinBtn);
            menu.appendChild(delBtn);

            actions.appendChild(ellipsisBtn);
            actions.appendChild(menu);

            ellipsisBtn.onclick = (e) => {
                e.stopPropagation();
                document.querySelectorAll('.sidebar-item-menu').forEach(m => {
                    if (m !== menu) m.classList.add('hidden');
                });
                menu.classList.toggle("hidden");
            };

            item.appendChild(titleEl);
            item.appendChild(actions);

            if (chat.pinned) {
                pinnedList.appendChild(item);
                hasPinned = true;
            } else {
                recentList.appendChild(item);
            }
        });

        sectionPinned.style.display = hasPinned ? "block" : "none";

    } catch (e) {
        console.error("Failed to load sidebar history:", e);
    }
}

async function autoSaveSession() {
    if (!currentSessionId) {
        currentSessionId = generateUUID();
        // If it's a new session and we have at least 1 user node, generate title
        const userNodes = Object.values(chatTree).filter(n => n.role === "user");
        if (userNodes.length > 0) {
            generateSessionTitle(userNodes[0].content);
        }
    }

    try {
        await fetch(`${API_BASE}/history/save`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                id: currentSessionId,
                title: currentSessionTitle,
                pinned: isSessionPinned,
                updated_at: Date.now(),
                active_node_id: activeNodeId,
                tree_data: JSON.stringify(chatTree)
            })
        });
        loadSidebarHistory();
    } catch (e) {
        console.error("Failed to auto-save session:", e);
    }
}

async function loadSession(id) {
    if (isProcessing) return; // don't load while generating

    try {
        const res = await fetch(`${API_BASE}/history/load/${id}`);
        if (!res.ok) return;
        const raw = await res.json();

        currentSessionId = raw.id;
        currentSessionTitle = raw.title;
        isSessionPinned = raw.pinned;

        if (raw.tree_data) {
            chatTree = JSON.parse(raw.tree_data);
            activeNodeId = raw.active_node_id;
            renderBranch(activeNodeId);
        } else {
            chatTree = {};
            activeNodeId = null;
            document.getElementById("messages").innerHTML = "";
            document.getElementById("chat-container").classList.add("chat-empty");
            if (welcomeEl) welcomeEl.style.display = "";
        }

        loadSidebarHistory();

        // Hide sidebar on mobile after clicking
        if (window.innerWidth < 768) {
            toggleSidebar(false); // assuming toggleSidebar logic exists
        }
        updateTopbar();
        if (inputEl) inputEl.focus();
    } catch (e) {
        console.error("Failed to load session:", e);
    }
}

function startNewSession() {
    if (isProcessing) return;
    currentSessionId = null;
    currentSessionTitle = "New Chat";
    isSessionPinned = false;
    chatTree = {};
    activeNodeId = null;

    document.getElementById("messages").innerHTML = "";
    document.getElementById("chat-container").classList.add("chat-empty");
    if (welcomeEl) welcomeEl.style.display = "flex";

    loadSidebarHistory();
    updateTopbar();
    if (inputEl) inputEl.focus();
}

async function generateSessionTitle(prompt) {
    try {
        const payload = {
            model: currentModel,
            message: `Generate a very short, concise 3-5 word title for a conversation that starts with the following prompt. ONLY output the title, no quotes or additional text. Prompt: ${prompt}`
        };
        console.log("Generating title using payload:", payload);
        const res = await fetch(`${API_BASE}/chat/send`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload)
        });

        if (!res.ok) {
            console.error("Title generation rejected. Status:", res.status);
            const errBody = await res.text();
            console.error(errBody);
            return;
        }

        const data = await res.json();
        console.log("Title generation got chat_id:", data.chat_id);

        const evtSource = new EventSource(`${API_BASE}/chat/events/${data.chat_id}`);
        let titleChunk = "";

        evtSource.addEventListener("text", (e) => {
            try {
                const payload = JSON.parse(e.data);
                console.log("Title SSE text chunk:", payload);
                titleChunk += payload.content || "";
            } catch (err) { }
        });

        evtSource.addEventListener("done", () => {
            console.log("Title SSE complete. Final text:", titleChunk);
            evtSource.close();
            if (titleChunk.trim()) {
                currentSessionTitle = titleChunk.trim();
                updateTopbar();
                autoSaveSession(); // Resave with new title
                loadSidebarHistory(); // Update UI
            }
        });

        evtSource.addEventListener("error_msg", (e) => {
            console.error("Title SSE received explicit error_msg:", e);
            evtSource.close();
        });

        evtSource.onerror = (err) => {
            console.error("Title SSE connection error:", err);
            evtSource.close();
        };

    } catch (e) {
        console.warn("Exception generating title:", e);
    }
}

function getBranchPath(leafId) {
    const path = [];
    let curr = leafId;
    while (curr && chatTree[curr]) {
        path.unshift(chatTree[curr]);
        curr = chatTree[curr].parentId;
    }
    return path;
}

function getDeepestLeaf(nodeId) {
    let curr = chatTree[nodeId];
    while (curr && curr.children.length > 0) {
        curr = chatTree[curr.children[curr.children.length - 1]];
    }
    return curr ? curr.id : nodeId;
}

function buildGeminiHistory(leafId) {
    const fullPath = getBranchPath(leafId);
    if (!fullPath || fullPath.length === 0) return [];

    const RECENT_LIMIT = 15;
    const MEDIA_LIMIT = 4;

    // Split path into old and recent
    const recentIndex = Math.max(0, fullPath.length - RECENT_LIMIT);
    const oldNodes = fullPath.slice(0, recentIndex);
    const recentNodes = fullPath.slice(recentIndex);

    const history = [];

    // Phase 1: Aggregate keypoints from older history
    const aggregatedKeypoints = new Set();
    oldNodes.forEach(node => {
        if (node.model_keypoints) {
            node.model_keypoints.split(/[,;\n]/).forEach(kp => {
                const trimmed = kp.trim();
                if (trimmed) aggregatedKeypoints.add(trimmed);
            });
        }
    });

    // Phase 2: Add single Archival Memory block if we have old data
    if (aggregatedKeypoints.size > 0) {
        history.push({
            role: "model",
            parts: [{ text: `[ARCHIVAL MEMORY: Prior parts of this conversation covered: ${Array.from(aggregatedKeypoints).join(", ")}. Use these key points as context if needed, but stay focused on the user's latest message.]` }]
        });
    }

    // Phase 3: Process recent nodes
    for (let i = 0; i < recentNodes.length; i++) {
        const node = recentNodes[i];
        const isVeryRecentMedia = i >= (recentNodes.length - MEDIA_LIMIT);
        let parts = [];

        // Add media only if it's very recent to save tokens and prevent model "hallucinations" on stale visuals
        if (isVeryRecentMedia && node.images && node.images.length > 0) {
            node.images.forEach(img => {
                const partsMatch = img.match(/^data:([^;]+)(?:;name=[^;]+)?;base64,(.+)$/);
                if (partsMatch && partsMatch.length === 3) {
                    const rawMime = partsMatch[1];
                    const base64 = partsMatch[2];
                    let safeMime = toGeminiMime(rawMime);
                    if (!safeMime && rawMime.startsWith("audio/")) safeMime = rawMime;
                    if (!safeMime) return;

                    parts.push({
                        inlineData: {
                            mimeType: safeMime,
                            data: base64
                        }
                    });
                }
            });
        }

        // Add text content
        if (node.content) {
            // Discard error messages from history as they confuse the model
            if (node.role === "assistant" && node.content.startsWith("Error [")) continue;

            const part = { text: node.content };
            if (node.thoughtSignature) {
                part.thoughtSignature = node.thoughtSignature;
            }
            parts.push(part);
        } else if (node.role === "assistant" && node.thoughtSignature) {
            // Special case for tool-only nodes
            parts.push({
                text: "",
                thoughtSignature: node.thoughtSignature
            });
        }

        if (parts.length > 0) {
            history.push({
                role: node.role === "assistant" ? "model" : "user",
                parts: parts
            });
        }
    }

    return history;
}

function deleteNode(nodeId) {
    const node = chatTree[nodeId];
    if (!node) return;

    let variants;
    if (node.parentId && chatTree[node.parentId]) {
        const parent = chatTree[node.parentId];
        parent.children = parent.children.filter(id => id !== nodeId);
        variants = parent.children;
    } else {
        chatTree[nodeId].orphan = true;
        variants = Object.keys(chatTree).filter(k => chatTree[k].parentId === null && !chatTree[k].orphan);
    }

    let newActiveId = node.parentId;
    if (variants.length > 0) {
        newActiveId = variants[variants.length - 1];
    }

    if (newActiveId) {
        activeNodeId = getDeepestLeaf(newActiveId);
        renderBranch(activeNodeId);
    } else {
        activeNodeId = null;
        renderBranch(null);
    }
}

function renderBranch(leafId) {
    messagesEl.innerHTML = "";
    if (!leafId) {
        document.getElementById("chat-container").classList.add("chat-empty");
        welcomeEl.style.display = "flex";
        return;
    }
    const path = getBranchPath(leafId);
    path.forEach(node => {
        const contentEl = addMessageToDOM(node.role, node.content, [], node.images, node.id, true, node.info);
        if (node.extraNodesHTML) {
            const wrapper = document.createElement("div");
            wrapper.innerHTML = node.extraNodesHTML;
            Array.from(wrapper.children).forEach(child => contentEl.appendChild(child));
        }
    });
    scrollToBottom();
}

function addMessageToDOM(role, content, extraNodes = [], images = [], nodeId = null, isReplay = false, info = null) {
    const chatContainer = document.getElementById("chat-container");
    if (chatContainer.classList.contains("chat-empty")) {
        if (document.startViewTransition) {
            document.startViewTransition(() => {
                chatContainer.classList.remove("chat-empty");
            });
        } else {
            chatContainer.classList.remove("chat-empty");
        }
    }

    if (welcomeEl) {
        welcomeEl.style.display = "none";
    }

    const msgEl = document.createElement("div");
    msgEl.className = "message";
    if (nodeId) msgEl.setAttribute("data-id", nodeId);

    const innerEl = document.createElement("div");
    innerEl.className = "message-inner";

    const headerEl = document.createElement("div");
    headerEl.className = "message-header";

    const avatarEl = document.createElement("div");
    avatarEl.className = `message-avatar ${role}`;
    const googleName = isGoogleLoggedIn && googleUserInfo?.name ? googleUserInfo.name : null;
    const displayName = role === "user"
        ? (googleName || getSystemFullName())
        : currentModel;

    if (role === "user") {
        const googlePicture = isGoogleLoggedIn && googleUserInfo?.picture ? googleUserInfo.picture : null;
        if (googlePicture) {
            avatarEl.innerHTML = `<img src="${googlePicture}" alt="${displayName}" style="width:100%;height:100%;object-fit:cover;border-radius:50%;"/>`;
        } else if (userAvatarBase64) {
            avatarEl.innerHTML = `<img src="${userAvatarBase64}" alt="User Profile"/>`;
        } else {
            avatarEl.textContent = displayName.charAt(0).toUpperCase();
        }
    } else {
        avatarEl.innerHTML = `<svg width="22" height="22" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M17.3825 28.3045C22.4796 26.4903 26.4903 22.4796 28.3045 17.3825L31.0579 9.64686C31.3733 8.76063 32.6267 8.76063 32.9421 9.64686L35.6955 17.3825C37.5097 22.4796 41.5204 26.4903 46.6175 28.3045L54.3531 31.0579C55.2394 31.3733 55.2394 32.6267 54.3531 32.9421L46.6175 35.6955C41.5204 37.5097 37.5097 41.5204 35.6955 46.6175L32.9421 54.3531C32.6267 55.2394 31.3733 55.2394 31.0579 54.3531L28.3045 46.6175C26.4903 41.5204 22.4796 37.5097 17.3825 35.6955L9.64686 32.9421C8.76063 32.6267 8.76063 31.3733 9.64686 31.0579L17.3825 28.3045Z" fill="var(--accent)"/></svg>`;
    }

    const roleEl = document.createElement("span");
    roleEl.className = "message-role";
    roleEl.textContent = displayName;

    const now = new Date();
    const timeString = now.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
    const fullTimeString = now.toLocaleString();

    const timeEl = document.createElement("span");
    timeEl.className = "message-time";
    timeEl.textContent = ` - ${timeString}`;
    timeEl.title = fullTimeString;

    headerEl.appendChild(avatarEl);
    headerEl.appendChild(roleEl);
    headerEl.appendChild(timeEl);

    if (info) {
        const infoBadge = document.createElement("div");
        infoBadge.className = "message-info-badge";

        const timeIcon = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>`;
        const tokensIcon = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 2 7 12 12 22 7 12 2"></polygon><polyline points="2 17 12 22 22 17"></polyline><polyline points="2 12 12 17 22 12"></polyline></svg>`;

        infoBadge.innerHTML = `
            <span class="info-item" title="Tokens generated">${tokensIcon} ${info.tokens}</span>
            <span class="info-item" title="Time taken">${timeIcon} ${info.time}s</span>
        `;
        headerEl.appendChild(infoBadge);
    }

    // Add sticky action bar
    if (nodeId && chatTree[nodeId]) {
        const node = chatTree[nodeId];
        const actionBar = createActionBar(node, role);
        headerEl.appendChild(actionBar);
    }

    const contentEl = document.createElement("div");
    contentEl.className = "message-content";

    // Render attached files at top of message
    if (images.length > 0) {
        const imageRow = document.createElement("div");
        imageRow.className = "message-images";
        images.forEach(imgSrc => {
            if (imgSrc.startsWith("data:image/")) {
                const thumb = document.createElement("img");
                thumb.className = "message-image-thumb";
                thumb.src = imgSrc;
                thumb.alt = "Attached image";
                thumb.addEventListener("click", () => openImageLightbox(imgSrc));
                imageRow.appendChild(thumb);
            } else if (imgSrc.startsWith("data:audio/")) {
                const nameMatch = imgSrc.match(/name=([^;]+)/);
                const title = nameMatch ? decodeURIComponent(nameMatch[1]) : "Ses Kaydı";
                const audioWrap = createCustomAudioPlayer(imgSrc, title);
                imageRow.appendChild(audioWrap);
            } else if (imgSrc.startsWith("data:video/")) {
                const videoWrap = document.createElement("div");
                videoWrap.className = "message-video-player";
                const video = document.createElement("video");
                video.controls = true;
                video.src = imgSrc;
                video.preload = "metadata";
                video.style.maxWidth = "320px";
                video.style.borderRadius = "8px";
                videoWrap.appendChild(video);
                imageRow.appendChild(videoWrap);
            } else {
                // Generic file chip
                const mimeLine = imgSrc.split(";")[0];
                const mimeType = mimeLine.replace("data:", "");
                const ext = mimeType.split("/").pop().substring(0, 4) || "FILE";

                const fileChip = document.createElement("div");
                fileChip.className = "message-file-chip";
                fileChip.innerHTML = `
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                        <polyline points="14 2 14 8 20 8"></polyline>
                        <line x1="16" y1="13" x2="8" y2="13"></line>
                        <line x1="16" y1="17" x2="8" y2="17"></line>
                        <polyline points="10 9 9 9 8 9"></polyline>
                    </svg>
                    <span>${ext.toUpperCase()} Ek</span>
                `;
                imageRow.appendChild(fileChip);
            }
        });
        contentEl.appendChild(imageRow);
    }

    if (extraNodes.length > 0) {
        extraNodes.forEach(child => contentEl.appendChild(child));
    }

    const markdownNode = document.createElement("div");
    markdownNode.innerHTML = renderMarkdown(content);
    if (role === "assistant") {
        addCopyButtons(markdownNode);
    }
    contentEl.appendChild(markdownNode);

    innerEl.appendChild(headerEl);
    innerEl.appendChild(contentEl);
    msgEl.appendChild(innerEl);
    messagesEl.appendChild(msgEl);

    if (!isReplay) {
        scrollToBottom();
    }

    return contentEl;
}

function createCustomAudioPlayer(src, title = "Ses Kaydı") {
    const wrap = document.createElement("div");
    wrap.className = "custom-audio-player";

    // Header
    const header = document.createElement("div");
    header.className = "audio-player-header";

    const titleEl = document.createElement("div");
    titleEl.className = "audio-player-title";
    titleEl.textContent = title;

    const downloadBtn = document.createElement("a");
    downloadBtn.className = "audio-download-btn";
    downloadBtn.href = src;
    downloadBtn.download = title;
    downloadBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>`;

    header.appendChild(titleEl);
    header.appendChild(downloadBtn);

    // Controls container
    const controlsWrap = document.createElement("div");
    controlsWrap.className = "audio-player-controls";

    const audio = document.createElement("audio");
    audio.src = src;
    audio.preload = "metadata";

    const playBtn = document.createElement("button");
    playBtn.className = "audio-play-btn";
    playBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`;

    const timeDisplay = document.createElement("span");
    timeDisplay.className = "audio-time";
    timeDisplay.textContent = "0:00";

    const formatTime = (timeInSecs) => {
        if (!timeInSecs || isNaN(timeInSecs)) return "0:00";
        if (!isFinite(timeInSecs)) return "0:00";
        const mins = Math.floor(timeInSecs / 60);
        const secs = Math.floor(timeInSecs % 60);
        return `${mins}:${secs.toString().padStart(2, '0')}`;
    };

    // Use a stylized flex wave for the track
    const trackArea = document.createElement("div");
    trackArea.className = "audio-track-area";

    // Fixed aesthetic pattern so players remain 1:1 visually matching
    const wavePattern = [30, 50, 80, 45, 60, 100, 70, 55, 90, 40, 65, 85, 30, 50, 75, 40, 60, 35, 70, 45, 80];
    const barsCount = wavePattern.length;
    for (let i = 0; i < barsCount; i++) {
        const bar = document.createElement("div");
        bar.className = "audio-wave-bar";
        bar.style.height = `${wavePattern[i]}%`;
        trackArea.appendChild(bar);
    }

    const speedBtn = document.createElement("button");
    speedBtn.className = "audio-speed-btn";
    speedBtn.textContent = "1x";

    let currentSpeed = 1;
    speedBtn.onclick = (e) => {
        e.stopPropagation();
        const speeds = [1, 1.25, 1.5, 2];
        const nextIndex = (speeds.indexOf(currentSpeed) + 1) % speeds.length;
        currentSpeed = speeds[nextIndex];
        audio.playbackRate = currentSpeed;
        speedBtn.textContent = `${currentSpeed}x`;
    };

    let isPlaying = false;

    playBtn.onclick = (e) => {
        e.stopPropagation();
        if (isPlaying) {
            audio.pause();
        } else {
            audio.play();
        }
    };

    audio.addEventListener('play', () => {
        isPlaying = true;
        playBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"></rect><rect x="14" y="4" width="4" height="16"></rect></svg>`;
    });

    audio.addEventListener('pause', () => {
        isPlaying = false;
        playBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`;
    });

    audio.addEventListener('timeupdate', () => {
        timeDisplay.textContent = formatTime(audio.currentTime);
        if (audio.duration && isFinite(audio.duration)) {
            const percent = (audio.currentTime / audio.duration) * 100;
            // Update the bars to fill based on percentage
            const bars = trackArea.querySelectorAll('.audio-wave-bar');
            const activeCount = Math.floor((percent / 100) * barsCount);
            bars.forEach((bar, idx) => {
                if (idx < activeCount) {
                    bar.classList.add("played");
                } else {
                    bar.classList.remove("played");
                }
            });
        }
    });

    audio.addEventListener('loadedmetadata', () => {
        // Chromium bug: recorded webm blobs have Infinity duration.
        // We force standard duration indexing by scrubbing way out of bounds.
        if (!isFinite(audio.duration)) {
            // Scrubbing to arbitrary huge number resolves the max timeline boundary
            audio.currentTime = 1e6;
        } else {
            timeDisplay.textContent = formatTime(audio.duration);
        }
    });

    audio.addEventListener('durationchange', () => {
        if (isFinite(audio.duration)) {
            if (audio.currentTime > 1e5) {
                // Return to start after our hack resolves boundary
                audio.currentTime = 0;
            }
            timeDisplay.textContent = formatTime(audio.duration);
        }
    });

    audio.addEventListener('ended', () => {
        isPlaying = false;
        playBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`;
        const bars = trackArea.querySelectorAll('.audio-wave-bar');
        bars.forEach(bar => bar.classList.remove("played"));
        if (isFinite(audio.duration)) {
            timeDisplay.textContent = formatTime(audio.duration);
        }
    });

    trackArea.onclick = (e) => {
        e.stopPropagation();
        if (audio.duration && isFinite(audio.duration)) {
            const rect = trackArea.getBoundingClientRect();
            const pos = (e.clientX - rect.left) / rect.width;
            audio.currentTime = pos * audio.duration;
        }
    };

    controlsWrap.appendChild(audio);
    controlsWrap.appendChild(playBtn);
    controlsWrap.appendChild(timeDisplay);
    controlsWrap.appendChild(trackArea);
    controlsWrap.appendChild(speedBtn);

    wrap.appendChild(header);
    wrap.appendChild(controlsWrap);

    return wrap;
}

function addMessage(role, content, extraNodes = [], images = [], info = null, thoughtSignature = null) {
    let extraNodesHTML = "";
    if (extraNodes.length > 0) {
        const wrapper = document.createElement("div");
        extraNodes.forEach(node => wrapper.appendChild(node.cloneNode(true)));
        extraNodesHTML = wrapper.innerHTML;
    }
    const node = createNode(role, content, activeNodeId, extraNodesHTML, images, info, thoughtSignature);
    activeNodeId = node.id;
    return addMessageToDOM(role, content, extraNodes, images, node.id, false, info);
}

function createActionBar(node, role) {
    const container = document.createElement("div");
    container.className = "message-actions-container";

    const variants = node.parentId ? chatTree[node.parentId].children : Object.keys(chatTree).filter(k => chatTree[k].parentId === null && !chatTree[k].orphan);
    if (variants.length > 1) {
        const idx = variants.indexOf(node.id);
        const pageEl = document.createElement("div");
        pageEl.className = "action-pagination";

        const prevBtn = document.createElement("button");
        prevBtn.innerHTML = "&lt;";
        prevBtn.disabled = idx === 0;
        prevBtn.onclick = () => {
            if (isProcessing) return;
            if (idx > 0) {
                activeNodeId = getDeepestLeaf(variants[idx - 1]);
                renderBranch(activeNodeId);
            }
        };

        const nextBtn = document.createElement("button");
        nextBtn.innerHTML = "&gt;";
        nextBtn.disabled = idx === variants.length - 1;
        nextBtn.onclick = () => {
            if (isProcessing) return;
            if (idx < variants.length - 1) {
                activeNodeId = getDeepestLeaf(variants[idx + 1]);
                renderBranch(activeNodeId);
            }
        };

        const label = document.createElement("span");
        label.textContent = `${idx + 1} / ${variants.length}`;

        pageEl.appendChild(prevBtn);
        pageEl.appendChild(label);
        pageEl.appendChild(nextBtn);
        container.appendChild(pageEl);
    }

    const btnGroup = document.createElement("div");
    btnGroup.className = "action-btn-group";

    if (role === "user") {
        const editBtn = document.createElement("button");
        editBtn.className = "msg-action-btn icon-only";
        editBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"></path><path d="M16.5 3.5a2.121 2 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path></svg>`;
        editBtn.title = "Edit";
        editBtn.onclick = (e) => {
            if (isProcessing) return;
            const messageEl = e.target.closest('.message');
            const contentContainer = messageEl.querySelector('.message-content');
            if (contentContainer.querySelector('.inline-edit-container')) return;

            const markdownNodes = Array.from(contentContainer.children).filter(c => !c.classList.contains('message-images'));
            markdownNodes.forEach(n => n.style.display = 'none');

            const actionBar = messageEl.querySelector('.message-actions-container');
            if (actionBar) actionBar.style.display = 'none';

            const editContainer = document.createElement('div');
            editContainer.className = 'inline-edit-container';

            const textarea = document.createElement('textarea');
            textarea.className = 'inline-edit-textarea';
            textarea.value = node.content;

            const resizeTextarea = () => {
                textarea.style.height = 'auto';
                textarea.style.height = Math.max(56, textarea.scrollHeight) + 'px';
            };
            textarea.addEventListener('input', resizeTextarea);
            setTimeout(resizeTextarea, 0);

            const btnRow = document.createElement('div');
            btnRow.className = 'inline-edit-actions';

            const cancelBtn = document.createElement('button');
            cancelBtn.className = 'inline-edit-cancel';
            cancelBtn.textContent = 'Cancel';
            cancelBtn.onclick = () => {
                editContainer.remove();
                markdownNodes.forEach(n => n.style.display = '');
                if (actionBar) actionBar.style.display = '';
            };

            const saveBtn = document.createElement('button');
            saveBtn.className = 'inline-edit-save';
            saveBtn.textContent = 'Save & Submit';
            saveBtn.onclick = () => {
                const newText = textarea.value.trim();
                if (!newText) return;

                editContainer.remove();
                markdownNodes.forEach(n => n.style.display = '');
                if (actionBar) actionBar.style.display = '';

                activeNodeId = node.parentId;
                let overrideImages = null;
                if (node.images && node.images.length > 0) {
                    const match = node.images[0].match(/^data:([^;]+(?:;name=[^;]+)?);base64,(.*)$/);
                    if (match) { overrideImages = { base64: match[2], mime: match[1].replace(/;name=[^;]+/, ''), fullArray: node.images }; }
                }
                sendMessage(newText, overrideImages);
            };

            btnRow.appendChild(cancelBtn);
            btnRow.appendChild(saveBtn);
            editContainer.appendChild(textarea);
            editContainer.appendChild(btnRow);

            contentContainer.appendChild(editContainer);
            textarea.focus();
            textarea.selectionStart = textarea.selectionEnd = textarea.value.length;
        };

        const rerunBtn = document.createElement("button");
        rerunBtn.className = "msg-action-btn icon-only";
        rerunBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="1 4 1 10 7 10"></polyline><polyline points="23 20 23 14 17 14"></polyline><path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10M23 14l-4.64 4.36A9 9 0 0 1 3.51 15"></path></svg>`;
        rerunBtn.title = "Rerun";
        rerunBtn.onclick = (e) => {
            if (isProcessing) return;
            activeNodeId = node.parentId;
            let overrideImages = null;
            if (node.images && node.images.length > 0) {
                const match = node.images[0].match(/^data:([^;]+(?:;name=[^;]+)?);base64,(.*)$/);
                if (match) { overrideImages = { base64: match[2], mime: match[1].replace(/;name=[^;]+/, ''), fullArray: node.images }; }
            }
            sendMessage(node.content, overrideImages);
        };

        const copyBtn = document.createElement("button");
        copyBtn.className = "msg-action-btn icon-only";
        copyBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>`;
        copyBtn.title = "Copy";
        copyBtn.onclick = (e) => {
            navigator.clipboard.writeText(node.content);
            showToast("Copied to clipboard", "info");
        };

        btnGroup.appendChild(editBtn);
        btnGroup.appendChild(rerunBtn);
        btnGroup.appendChild(copyBtn);

    } else {
        const rerunBtn = document.createElement("button");
        rerunBtn.className = "msg-action-btn icon-only";
        rerunBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="1 4 1 10 7 10"></polyline><polyline points="23 20 23 14 17 14"></polyline><path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10M23 14l-4.64 4.36A9 9 0 0 1 3.51 15"></path></svg>`;
        rerunBtn.title = "Regenerate";
        rerunBtn.onclick = (e) => {
            if (isProcessing) return;
            const userParent = chatTree[node.parentId];
            if (userParent) {
                activeNodeId = userParent.id;
                let overrideImages = null;
                if (userParent.images && userParent.images.length > 0) {
                    const match = userParent.images[0].match(/^data:([^;]+(?:;name=[^;]+)?);base64,(.*)$/);
                    if (match) { overrideImages = { base64: match[2], mime: match[1].replace(/;name=[^;]+/, ''), fullArray: userParent.images }; }
                }
                sendMessage(userParent.content, overrideImages, true);
            }
        };

        const deleteBtn = document.createElement("button");
        deleteBtn.className = "msg-action-btn danger icon-only";
        deleteBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>`;
        deleteBtn.title = "Delete";
        deleteBtn.onclick = (e) => {
            if (isProcessing) return;
            deleteNode(node.id);
        };

        btnGroup.appendChild(rerunBtn);
        btnGroup.appendChild(deleteBtn);
    }

    container.appendChild(btnGroup);
    return container;
}

// Fullscreen image lightbox
function openImageLightbox(src) {
    let overlay = document.getElementById("image-lightbox");
    if (!overlay) {
        overlay = document.createElement("div");
        overlay.id = "image-lightbox";
        overlay.innerHTML = `
            <button class="lightbox-close" id="lightbox-close">&times;</button>
            <img class="lightbox-img" id="lightbox-img" draggable="false" />
        `;
        document.body.appendChild(overlay);

        const closeBtn = overlay.querySelector("#lightbox-close");
        const img = overlay.querySelector("#lightbox-img");

        let scale = 1;
        let translateX = 0;
        let translateY = 0;
        let isDragging = false;
        let dragStartX = 0;
        let dragStartY = 0;
        let dragStartTX = 0;
        let dragStartTY = 0;

        function applyTransform() {
            img.style.transform = `translate(${translateX}px, ${translateY}px) scale(${scale})`;
        }

        // Close button
        closeBtn.addEventListener("click", (e) => {
            e.stopPropagation();
            overlay.classList.remove("active");
        });

        // Click background to close (only if not zoomed)
        overlay.addEventListener("click", (e) => {
            if (e.target === overlay && scale <= 1) {
                overlay.classList.remove("active");
            }
        });

        // Click image to toggle zoom
        img.addEventListener("click", (e) => {
            e.stopPropagation();
            if (!isDragging) {
                if (scale <= 1) {
                    scale = 2.5;
                    translateX = 0;
                    translateY = 0;
                } else {
                    scale = 1;
                    translateX = 0;
                    translateY = 0;
                }
                applyTransform();
                img.style.cursor = scale > 1 ? "grab" : "zoom-in";
            }
        });

        // Mouse drag to pan when zoomed
        img.addEventListener("mousedown", (e) => {
            if (scale > 1) {
                e.preventDefault();
                isDragging = false;
                dragStartX = e.clientX;
                dragStartY = e.clientY;
                dragStartTX = translateX;
                dragStartTY = translateY;
                img.style.cursor = "grabbing";
                img.style.transition = "none"; // Instant movement

                const onMove = (ev) => {
                    const dx = ev.clientX - dragStartX;
                    const dy = ev.clientY - dragStartY;
                    if (Math.abs(dx) > 3 || Math.abs(dy) > 3) isDragging = true;
                    translateX = dragStartTX + dx;
                    translateY = dragStartTY + dy;
                    applyTransform();
                };

                const onUp = () => {
                    document.removeEventListener("mousemove", onMove);
                    document.removeEventListener("mouseup", onUp);
                    img.style.transition = "transform 0.2s ease"; // Restore
                    img.style.cursor = scale > 1 ? "grab" : "zoom-in";
                    setTimeout(() => { isDragging = false; }, 10);
                };

                document.addEventListener("mousemove", onMove);
                document.addEventListener("mouseup", onUp);
            }
        });

        // Scroll wheel zoom
        overlay.addEventListener("wheel", (e) => {
            e.preventDefault();
            scale += e.deltaY * -0.003;
            scale = Math.min(Math.max(0.5, scale), 8);
            if (scale <= 1) { translateX = 0; translateY = 0; }
            applyTransform();
            img.style.cursor = scale > 1 ? "grab" : "zoom-in";
        }, { passive: false });

        // Reset state
        overlay._reset = () => {
            scale = 1;
            translateX = 0;
            translateY = 0;
            isDragging = false;
            img.style.transform = "translate(0px, 0px) scale(1)";
            img.style.cursor = "zoom-in";
        };
    }

    overlay._reset();
    overlay.querySelector("#lightbox-img").src = src;
    overlay.classList.add("active");
}

function showToast(message, type = 'info') {
    const container = document.getElementById("toast-container");
    if (!container) return;

    const toast = document.createElement("div");
    toast.className = `toast toast-${type}`;

    let icon = '';
    if (type === 'error') {
        icon = `<svg class="toast-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>`;
    } else if (type === 'warning') {
        icon = `<svg class="toast-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>`;
    } else {
        icon = `<svg class="toast-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>`;
    }

    toast.innerHTML = `${icon}<span>${message}</span>`;
    container.appendChild(toast);

    setTimeout(() => {
        toast.classList.add("removing");
        toast.addEventListener("animationend", () => {
            toast.remove();
        });
    }, 5000);
}

async function fetchModels() {
    try {
        const res = await fetch(`${API_BASE}/chat/models`);
        if (!res.ok) {
            const loadingRow = document.getElementById("model-loading-row");
            if (loadingRow) loadingRow.querySelector("span").textContent = "No models available";
            return;
        }
        const data = await res.json();
        const models = data.models || [];
        if (models.length === 0) {
            const loadingRow = document.getElementById("model-loading-row");
            if (loadingRow) loadingRow.querySelector("span").textContent = "No models available";
            return;
        }

        const popover = document.getElementById("model-popover");
        const divider = popover.querySelector(".popover-divider");

        // Remove loading row if present
        const loadingRow = document.getElementById("model-loading-row");
        if (loadingRow) loadingRow.remove();

        let current = popover.firstChild;
        while (current && current !== divider) {
            let next = current.nextSibling;
            if (current.classList && current.classList.contains("model-option")) {
                popover.removeChild(current);
            }
            current = next;
        }

        const moreSection = document.getElementById("more-models-section");
        moreSection.innerHTML = "";

        // Sort prioritized: prefer 3.1 > 2.5 > 2.0 > 1.5 (newest first)
        const scoreName = (name) => {
            if (name.includes('3.1')) return 4;
            if (name.includes('2.5')) return 3;
            if (name.includes('2.0')) return 2;
            if (name.includes('1.5')) return 1;
            return 0;
        };

        function createModelOption(model, isActive = false) {
            const div = document.createElement("div");
            div.className = `model-option${isActive ? " active" : ""}`;
            const shortName = model.name.split("/").pop();
            div.setAttribute("data-model", shortName);

            const displayName = model.display_name || shortName;
            const rawDesc = model.description || '';
            const desc = rawDesc.length > 60 ? rawDesc.substring(0, 60) + '…' : rawDesc;

            div.innerHTML = `
                <div class="model-header">
                    <span class="model-name">${displayName}</span>
                </div>
                <div class="model-desc">${desc}</div>
                <svg class="check-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
            `;

            div.addEventListener("click", () => {
                selectModel(div);
            });

            return div;
        }


        const allSorted = [...models].sort((a, b) => scoreName(b.name) - scoreName(a.name));

        // Top 3 go in main list, rest go under More models
        const topModels = allSorted.slice(0, 3);
        const moreModels = allSorted.slice(3);

        topModels.forEach((m) => {
            const shortName = m.name.split("/").pop();
            const div = createModelOption(m, shortName === currentModel);
            popover.insertBefore(div, divider);
        });

        // Auto-select the first model and update button label
        const firstModel = popover.querySelector(".model-option");
        if (firstModel) {
            selectModel(firstModel, false);
        }

        moreModels.forEach(m => {
            const shortName = m.name.split("/").pop();
            const div = createModelOption(m, shortName === currentModel);
            moreSection.appendChild(div);
        });

    } catch (err) {
        console.error("Failed to fetch models:", err);
        const loadingRow = document.getElementById("model-loading-row");
        if (loadingRow) loadingRow.querySelector("span").textContent = "Failed to load models";
    }
}

function selectModel(opt, hidePopover = true) {
    document.querySelectorAll(".model-option").forEach(o => o.classList.remove("active"));
    opt.classList.add("active");
    currentModel = opt.getAttribute("data-model");
    const textLabel = opt.querySelector(".model-name").textContent.trim();
    const labelEl = document.getElementById("model-btn-label");
    if (labelEl) labelEl.textContent = textLabel;
    if (hidePopover) modelPopover.classList.add("hidden");
}

function addTypingIndicator() {
    const chatContainer = document.getElementById("chat-container");
    if (chatContainer.classList.contains("chat-empty")) {
        if (document.startViewTransition) {
            document.startViewTransition(() => {
                chatContainer.classList.remove("chat-empty");
            });
        } else {
            chatContainer.classList.remove("chat-empty");
        }
    }


    const msgEl = document.createElement("div");
    msgEl.className = "message";
    msgEl.id = "typing-message";

    const innerEl = document.createElement("div");
    innerEl.className = "message-inner";

    const headerEl = document.createElement("div");
    headerEl.className = "message-header";

    const avatarEl = document.createElement("div");
    avatarEl.className = "message-avatar assistant";
    const displayName = currentModel;
    avatarEl.innerHTML = `<svg width="22" height="22" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M17.3825 28.3045C22.4796 26.4903 26.4903 22.4796 28.3045 17.3825L31.0579 9.64686C31.3733 8.76063 32.6267 8.76063 32.9421 9.64686L35.6955 17.3825C37.5097 22.4796 41.5204 26.4903 46.6175 28.3045L54.3531 31.0579C55.2394 31.3733 55.2394 32.6267 54.3531 32.9421L46.6175 35.6955C41.5204 37.5097 37.5097 41.5204 35.6955 46.6175L32.9421 54.3531C32.6267 55.2394 31.3733 55.2394 31.0579 54.3531L28.3045 46.6175C26.4903 41.5204 22.4796 37.5097 17.3825 35.6955L9.64686 32.9421C8.76063 32.6267 8.76063 31.3733 9.64686 31.0579L17.3825 28.3045Z" fill="var(--accent)"/></svg>`;

    const roleEl = document.createElement("span");
    roleEl.className = "message-role";
    roleEl.textContent = displayName;

    const now = new Date();
    const timeString = now.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
    const fullTimeString = now.toLocaleString();

    const timeEl = document.createElement("span");
    timeEl.className = "message-time";
    timeEl.textContent = ` - ${timeString}`;
    timeEl.title = fullTimeString;

    headerEl.appendChild(avatarEl);
    headerEl.appendChild(roleEl);
    headerEl.appendChild(timeEl);

    const contentEl = document.createElement("div");
    contentEl.className = "message-content";

    // Phase 1: Connecting indicator with three dots
    const connectingEl = document.createElement("div");
    connectingEl.className = "connecting-indicator";
    connectingEl.id = "connecting-phase";
    connectingEl.innerHTML = `
        <div class="dot-group">
            <span></span><span></span><span></span>
        </div>
        <span class="status-text">${t("status.connecting")}</span>
    `;

    contentEl.appendChild(connectingEl);
    innerEl.appendChild(headerEl);
    innerEl.appendChild(contentEl);
    msgEl.appendChild(innerEl);
    messagesEl.appendChild(msgEl);
    scrollToBottom();

    return contentEl;
}

function switchToThinkingPhase() {
    const connectingEl = document.getElementById("connecting-phase");
    if (!connectingEl) return;

    const thinkingEl = document.createElement("div");
    thinkingEl.className = "thinking-indicator";
    thinkingEl.id = "thinking-phase";
    thinkingEl.innerHTML = `
        <svg class="think-icon" viewBox="0 0 24 24">
            <circle class="think-circle" cx="12" cy="12" r="6" />
            <circle class="think-dot" cx="9" cy="12" r="1.2" />
            <circle class="think-dot" cx="12" cy="12" r="1.2" />
            <circle class="think-dot" cx="15" cy="12" r="1.2" />
        </svg>
        <span class="status-text">Düşünüyor...</span>
    `;

    connectingEl.replaceWith(thinkingEl);
}

function addThinkingSection(parentEl, thoughtText) {
    // Parse thought text into lines for timeline
    const lines = thoughtText.split('\n').filter(l => l.trim());
    const summaryLine = lines[0] || 'Düşünüyor...';
    const detailLines = lines.slice(1);

    // Check if thought chain already exists
    let chainEl = parentEl.querySelector(".thought-chain");
    if (chainEl) {
        // Update existing
        const summaryEl = chainEl.querySelector(".thought-summary");
        const bodyEl = chainEl.querySelector(".thought-chain-body");
        summaryEl.innerHTML = marked.parseInline(summaryLine);
        bodyEl.innerHTML = '';
        const timelineEl = buildTimeline(detailLines);
        bodyEl.appendChild(timelineEl);
        return;
    }

    chainEl = document.createElement("div");
    chainEl.className = "thought-chain";

    // Header: sparkle + summary + chevron
    const headerEl = document.createElement("div");
    headerEl.className = "thought-chain-header";

    const sparkle = `<svg class="thought-sparkle" width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2L14.09 8.26L20 9.27L15.55 13.97L16.91 20L12 16.9L7.09 20L8.45 13.97L4 9.27L9.91 8.26L12 2Z"/></svg>`;

    const summaryEl = document.createElement("span");
    summaryEl.className = "thought-summary";
    summaryEl.innerHTML = marked.parseInline(summaryLine);

    const chevron = `<svg class="thought-chevron" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg>`;

    headerEl.innerHTML = sparkle;
    headerEl.appendChild(summaryEl);
    headerEl.insertAdjacentHTML("beforeend", chevron);

    // Body: vertical timeline
    const bodyEl = document.createElement("div");
    bodyEl.className = "thought-chain-body";
    const timelineEl = buildTimeline(detailLines);
    bodyEl.appendChild(timelineEl);

    headerEl.addEventListener("click", () => {
        headerEl.classList.toggle("expanded");
        bodyEl.classList.toggle("visible");
    });

    chainEl.appendChild(headerEl);
    chainEl.appendChild(bodyEl);

    // Insert before any existing indicator
    const indicator = parentEl.querySelector(".connecting-indicator, .thinking-indicator");
    if (indicator) {
        parentEl.insertBefore(chainEl, indicator);
    } else {
        parentEl.appendChild(chainEl);
    }

    scrollToBottom();
}

function buildTimeline(lines) {
    const timelineEl = document.createElement("div");
    timelineEl.className = "thought-timeline";

    const icons = [
        `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>`,
        `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>`,
        `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>`,
        `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>`,
        `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`,
    ];

    lines.forEach((line, idx) => {
        const item = document.createElement("div");
        item.className = "thought-timeline-item";

        const iconEl = document.createElement("div");
        iconEl.className = "thought-timeline-icon";
        iconEl.innerHTML = icons[idx % icons.length];

        const textEl = document.createElement("div");
        textEl.className = "thought-timeline-text";
        textEl.innerHTML = marked.parseInline(line.trim());

        item.appendChild(iconEl);
        item.appendChild(textEl);
        timelineEl.appendChild(item);
    });

    return timelineEl;
}

function removeTypingIndicator() {
    const el = document.getElementById("typing-message");
    if (el) el.remove();
}

function addToolCall(parentEl, name, args, status) {
    const toolEl = document.createElement("div");
    toolEl.className = "tool-call";

    const headerEl = document.createElement("div");
    headerEl.className = "tool-call-header";

    const chevron = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg>`;

    const nameEl = document.createElement("span");
    nameEl.className = "tool-call-name";
    nameEl.textContent = name;

    const statusEl = document.createElement("span");
    statusEl.className = "tool-call-status";
    statusEl.textContent = status || "running...";

    headerEl.innerHTML = chevron;
    headerEl.appendChild(nameEl);
    headerEl.appendChild(statusEl);

    const bodyEl = document.createElement("div");
    bodyEl.className = "tool-call-body";
    const argsStr = typeof args === "string" ? args : JSON.stringify(args, null, 2);
    bodyEl.innerHTML = `<pre><code class="hljs language-json">${hljs.highlight(argsStr, { language: 'json' }).value}</code></pre>`;

    headerEl.addEventListener("click", () => {
        headerEl.classList.toggle("expanded");
        bodyEl.classList.toggle("visible");
    });

    toolEl.appendChild(headerEl);
    toolEl.appendChild(bodyEl);

    const indicatorEl = parentEl.querySelector(".connecting-indicator, .thinking-indicator");
    if (indicatorEl) {
        parentEl.insertBefore(toolEl, indicatorEl);
    } else {
        parentEl.appendChild(toolEl);
    }

    scrollToBottom();

    return { statusEl, bodyEl };
}

function renderMarkdown(text) {
    // Hide memory tags from UI during streaming as well
    let cleanText = text.replace(/<memory>[\s\S]*?(?:<\/memory>|$)/i, "").trim();
    let html = marked.parse(cleanText);

    html = html.replace(/<pre><code(.*?)>/g, (match, attrs) => {
        let lang = "";
        const langMatch = attrs.match(/class="hljs language-(\w+)"/);
        if (langMatch) {
            lang = langMatch[1];
        }
        const header = `<div class="code-header"><span>${lang || "code"}</span><button class="copy-btn" onclick="copyCode(this)">Copy</button></div>`;
        return `<pre>${header}<code${attrs}>`;
    });

    return html;
}

function addCopyButtons(el) {
    el.querySelectorAll("pre code").forEach((block) => {
        block.parentElement.addEventListener("dblclick", () => {
            const text = block.textContent;
            navigator.clipboard.writeText(text);
            showToast("Copied to clipboard", "info");
        });
    });
}

window.copyCode = function (btn) {
    const pre = btn.closest("pre");
    const code = pre.querySelector("code");
    navigator.clipboard.writeText(code.textContent);
    btn.textContent = "Copied!";
    setTimeout(() => {
        btn.textContent = "Copy";
    }, 2000);
};

function scrollToBottom() {
    requestAnimationFrame(() => {
        messagesEl.scrollTop = messagesEl.scrollHeight;
    });
}

function autoResize() {
    inputEl.style.height = "auto";
    inputEl.style.height = Math.min(inputEl.scrollHeight, 150) + "px";
}

// Parse error strings like "Gemini API returned status 400 Bad Request: { "error": { ... } }"
function parseErrorMessage(rawError, fallbackCode) {
    let message = rawError;
    let code = fallbackCode || '';

    // Try direct JSON parse
    try {
        const parsed = JSON.parse(rawError);
        if (parsed.error) {
            message = parsed.error.message || rawError;
            code = parsed.error.code || fallbackCode;
            return { message, code };
        }
    } catch { /* not pure JSON */ }

    // Try extracting JSON from a prefix string like "... Bad Request: { ... }"
    const jsonMatch = rawError.match(/\{[\s\S]*"error"[\s\S]*\}/);
    if (jsonMatch) {
        try {
            const parsed = JSON.parse(jsonMatch[0]);
            if (parsed.error) {
                message = parsed.error.message || rawError;
                code = parsed.error.code || fallbackCode;
                return { message, code };
            }
        } catch { /* couldn't parse extracted JSON */ }
    }

    return { message, code };
}

// Send error to a reliable AI model for user-friendly explanation
async function explainErrorWithAI(errorCode, toastMsg, rawError) {
    // Guard: if we're already explaining an error, don't try again (prevents infinite loops)
    if (isExplainingError) {
        addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
        addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
        setProcessingState(false);
        return;
    }

    isExplainingError = true;
    const explanationModel = "gemini-2.0-flash";
    const prompt = t("errors.aiExplainPrompt", { errorCode, toastMsg, rawError });

    try {
        const explainRes = await fetch(`${API_BASE}/chat/send`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                message: prompt,
                model: explanationModel,
                thinking_level: "none",
                temperature: 0.7,
            }),
        });
        if (explainRes.ok) {
            const explainData = await explainRes.json();
            const chatId = explainData.chat_id;
            addTypingIndicator();
            const explainSource = new EventSource(`${API_BASE}/chat/events/${chatId}`);
            let explainText = '';
            explainSource.addEventListener("text", (ev) => {
                const p = JSON.parse(ev.data);
                explainText += p.content;
            });
            explainSource.addEventListener("done", () => {
                explainSource.close();
                removeTypingIndicator();
                if (explainText) addMessage("assistant", explainText);
                if (explainText) addMessage("assistant", explainText);
                setProcessingState(false);
                isExplainingError = false;
            });
            explainSource.addEventListener("error_msg", () => {
                explainSource.close();
                removeTypingIndicator();
                addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
                addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
                setProcessingState(false);
                isExplainingError = false;
            });
            explainSource.onerror = () => {
                explainSource.close();
                removeTypingIndicator();
                addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
                addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
                setProcessingState(false);
                isExplainingError = false;
            };
        } else {
            addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
            addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
            setProcessingState(false);
            isExplainingError = false;
        }
    } catch {
        addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
        addMessage("assistant", `Error [${errorCode}]: ${toastMsg}`);
        setProcessingState(false);
        isExplainingError = false;
    }
}

async function sendMessage(overrideText = null, overrideImages = null, isAiRerun = false) {
    const text = overrideText !== null ? overrideText : inputEl.value.trim();
    if (!text && !overrideImages && attachedFiles.length === 0) return;
    if (isProcessing) return;

    setProcessingState(true);
    if (overrideText === null) {
        inputEl.value = "";
        autoResize();
    }

    // Capture files for display before clearing
    const messageImages = [];
    const sendFiles = []; // Array of {base64, mime}

    if (overrideImages) {
        if (overrideImages.fullArray) {
            messageImages.push(...overrideImages.fullArray);
        } else {
            messageImages.push(`data:${overrideImages.mime};base64,${overrideImages.base64}`);
        }
        sendFiles.push({ base64: overrideImages.base64, mime: overrideImages.mime });
    } else if (attachedFiles.length > 0) {
        for (const f of attachedFiles) {
            let dataUrl = f.dataUrl;
            if (f.mimeType.startsWith('audio/') && f.name) {
                // If the user recorded audio or uploaded audio, embed the name in the data URL so we can render it later
                dataUrl = `data:${f.mimeType};name=${encodeURIComponent(f.name)};base64,${f.base64}`;
            }
            messageImages.push(dataUrl);
            sendFiles.push({ base64: f.base64, mime: f.mimeType });
        }

        // Clear attachments
        attachedFiles = [];
        imagePreview.classList.add("hidden");
        previewImg.src = "";
        previewImg.classList.remove("hidden");
        imageUpload.value = "";
        const fileIcon = document.getElementById("generic-file-icon");
        if (fileIcon) fileIcon.classList.add("hidden");
        // Clear multi-preview container
        const multiContainer = document.getElementById("multi-preview-container");
        if (multiContainer) multiContainer.innerHTML = "";
    }

    let historyNodeId = activeNodeId;
    if (isAiRerun && activeNodeId && chatTree[activeNodeId]) {
        historyNodeId = chatTree[activeNodeId].parentId;
    }

    const historyPayload = buildGeminiHistory(historyNodeId);

    // Sync visual DOM if appending to a historic branching point
    const displayedMessages = document.querySelectorAll('.message');
    const lastMessage = displayedMessages.length > 0 ? displayedMessages[displayedMessages.length - 1] : null;
    const isAtBottom = lastMessage && lastMessage.getAttribute('data-id') === activeNodeId;

    if (!isAtBottom && activeNodeId !== null) {
        renderBranch(activeNodeId);
    } else if (activeNodeId === null && displayedMessages.length > 0) {
        renderBranch(null);
    }

    if (!isAiRerun) {
        addMessage("user", text, [], messageImages);
    }



    const assistantContent = addTypingIndicator();

    // Start tracking time for the assistant node
    const startTimeStamp = Date.now();

    // Build the expanded payload
    const payload = {
        message: text,
        model: currentModel,
        thinking_level: currentThinkingLevel,
        temperature: parseFloat(tempSlider.value),
        enable_google_search: useSearch,
        enable_code_execution: useCode,
        history: historyPayload
    };

    const sysPrompt = systemPromptInput.value.trim();
    const memoryDirective = `\n\n[SYSTEM DIRECTIVE: At the very end of your response, you MUST include a <memory> tag containing 3-5 concise keywords or a short summary of the important concepts from this specific interaction. Example: <memory>Roblox Studio, Script injection, Event handling</memory>]`;

    payload.system_instruction = sysPrompt ? (sysPrompt + memoryDirective) : memoryDirective;

    if (sendFiles.length > 0) {
        payload.file_base64 = sendFiles.map(f => f.base64);
        payload.file_mime_type = sendFiles.map(f => f.mime);
    }

    let infoData = null;

    try {
        const res = await fetch(`${API_BASE}/chat/send`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(payload),
        });

        if (!res.ok) {
            const errText = await res.text();
            const { message: toastMessage, code: errorCode } = parseErrorMessage(errText, res.status);
            removeTypingIndicator();
            showToast(toastMessage, "error");

            // Send error to AI model for explanation using a reliable model
            await explainErrorWithAI(errorCode, toastMessage, errText);
            return;
        }

        const data = await res.json();
        currentChatId = data.chat_id;

        activeEventSource = new EventSource(`${API_BASE}/chat/events/${currentChatId}`);
        const evtSource = activeEventSource;

        let toolCalls = {};
        let finalText = "";
        let totalTokens = 0;
        let lastThoughtSignature = null;

        let switchedToThinking = false;

        evtSource.addEventListener("tool_call", (e) => {
            if (isAborting) return;
            if (!switchedToThinking) {
                switchToThinkingPhase();
                switchedToThinking = true;
            }
            const payload = JSON.parse(e.data);

            if (payload.name === "propose_plan") {
                const planText = payload.args.plan_text;
                // Add a placeholder block in the chat
                const refs = addToolCall(
                    assistantContent,
                    "Proposal: Review Plan",
                    {},
                    "Waiting for your review..."
                );
                toolCalls[payload.name + "_" + (payload.call_index || 0)] = refs;

                // Open the artifact sidebar
                openArtifactSidebar(planText, currentChatId);
                return;
            }

            const refs = addToolCall(
                assistantContent,
                payload.name,
                payload.args,
                "running..."
            );
            toolCalls[payload.name + "_" + (payload.call_index || 0)] = refs;
        });

        evtSource.addEventListener("tool_result", (e) => {
            if (isAborting) return;
            const payload = JSON.parse(e.data);

            if (payload.name === "propose_plan") {
                closeArtifactSidebar();
            }

            const key = payload.name + "_" + (payload.call_index || 0);
            const refs = toolCalls[key];
            if (refs) {
                refs.statusEl.textContent = "completed";
                const currentText = refs.bodyEl.textContent;
                const newText = currentText + "\n\n--- Result ---\n" + payload.result;
                refs.bodyEl.innerHTML = `<pre><code class="hljs language-json">${hljs.highlightAuto(newText).value}</code></pre>`;
            }
        });

        evtSource.addEventListener("thinking", (e) => {
            if (isAborting) return;
            if (!switchedToThinking) {
                switchToThinkingPhase();
                switchedToThinking = true;
            }
            const payload = JSON.parse(e.data);
            addThinkingSection(assistantContent, payload.content);
        });

        evtSource.addEventListener("thought_signature", (e) => {
            if (isAborting) return;
            const payload = JSON.parse(e.data);
            lastThoughtSignature = payload.signature;
            console.log("Captured thought signature:", lastThoughtSignature);
        });

        let liveTextNode = null;

        evtSource.addEventListener("text", (e) => {
            if (isAborting) return;
            if (!switchedToThinking) {
                switchToThinkingPhase();
                switchedToThinking = true;
            }

            // Hide indicators once text starts flowing
            const indicatorEl = assistantContent.querySelector(".connecting-indicator, .thinking-indicator");
            if (indicatorEl && indicatorEl.style.display !== "none") {
                indicatorEl.style.display = "none";
            }

            // Create streaming text container if it doesn't exist yet
            if (!liveTextNode) {
                liveTextNode = document.createElement("div");
                liveTextNode.className = "live-markdown-stream";
                assistantContent.appendChild(liveTextNode);
            }

            const payload = JSON.parse(e.data);
            finalText += payload.content;

            // Live render markdown to the DOM
            let displayString = finalText;
            const memoryMatch = displayString.match(/<memory>([\s\S]*?)(?:<\/memory>|$)/i);
            if (memoryMatch) {
                displayString = displayString.replace(/<memory>[\s\S]*?(?:<\/memory>|$)/i, "").trim();
            }
            liveTextNode.innerHTML = renderMarkdown(displayString);

            // In a streaming context we could add copy buttons on the fly, 
            // but it's cleaner to just let it finish.
            scrollToBottom();

            // Rough estimate mapping if exact tokens aren't streamed
            // Avg 4 chars per token
            totalTokens += Math.ceil(payload.content.length / 4);
            updateTopbar(totalTokens);
        });

        let hasError = false;

        evtSource.addEventListener("error_msg", async (e) => {
            hasError = true;
            evtSource.close();
            removeTypingIndicator();
            const payload = JSON.parse(e.data);
            const rawError = payload.error;
            const { message: toastMsg, code: errorCode } = parseErrorMessage(rawError, '');
            showToast(toastMsg, "error");

            // Send error to AI for explanation using a reliable model
            await explainErrorWithAI(errorCode, toastMsg, rawError);
        });

        finishGenerationHandler = () => {
            try {
                if (evtSource.readyState !== 2) {
                    evtSource.close();
                }

                const infoData = {
                    model: currentModel,
                    tokens: totalTokens,
                    time: ((Date.now() - startTimeStamp) / 1000).toFixed(1),
                    thoughtSignature: lastThoughtSignature
                };

                const typingMsg = document.getElementById("typing-message");
                const extraNodes = [];
                if (typingMsg) {
                    const nodesToKeep = typingMsg.querySelectorAll('.thought-chain, .tool-call');
                    nodesToKeep.forEach(node => {
                        extraNodes.push(node);
                    });
                }

                // Parse out the <memory> tag if it exists
                let displayString = finalText;
                let extractedMemory = null;
                const memoryMatch = displayString.match(/<memory>([\s\S]*?)<\/memory>/i);
                if (memoryMatch) {
                    extractedMemory = memoryMatch[1].trim();
                    displayString = displayString.replace(/<memory>[\s\S]*?<\/memory>/i, "").trim();
                }

                const finalNodes = Array.from(assistantContent.querySelectorAll('.thought-chain, .tool-call'));

                if (!displayString && !extractedMemory && finalNodes.length === 0 && !hasError && !isAborting) {
                    addMessage("assistant", "Empty response from stream or connection closed unexpectedly.", [], [], infoData, lastThoughtSignature);
                } else if (!hasError || isAborting) {
                    // Even if there's an error elsewhere, if we're aborting, just show what we have
                    addMessage("assistant", displayString, finalNodes, [], infoData, lastThoughtSignature);
                }

                if (activeNodeId && chatTree[activeNodeId]) {
                    chatTree[activeNodeId].info = infoData;
                    if (extractedMemory) {
                        chatTree[activeNodeId].model_keypoints = extractedMemory;
                    }
                    if (lastThoughtSignature) {
                        chatTree[activeNodeId].thoughtSignature = lastThoughtSignature;
                    }
                }

                if (!isAborting && !hasError) {
                    autoSaveSession();
                    updateTopbar();
                }

            } catch (e) {
                console.error("Error finishing generation:", e);
            } finally {
                removeTypingIndicator();
                setProcessingState(false);
                isAborting = false;
                activeEventSource = null;
                currentChatId = null;
                finishGenerationHandler = null;
                inputEl.focus();
            }
        };

        evtSource.addEventListener("done", finishGenerationHandler);

        // Removed error_msg block since it was moved above finishGenerationHandler to be in scope for `hasError`

        evtSource.onerror = () => {
            evtSource.close();
            removeTypingIndicator();
            if (!finalText && !hasError) {
                const msg = "Connection lost. Please try again.";
                addMessage("assistant", msg);
                showToast(msg, "error");
            }
            setProcessingState(false);
            activeEventSource = null;
            currentChatId = null;
        };
    } catch (err) {
        removeTypingIndicator();
        const msg = `Connection error: ${err.message}`;
        addMessage("assistant", msg);
        showToast(msg, "error");
        setProcessingState(false);
        activeEventSource = null;
        currentChatId = null;
    }
}

modelBtn.addEventListener("click", (e) => {
    e.stopPropagation();

    const rect = modelBtn.getBoundingClientRect();
    const titlebarHeight = document.getElementById("titlebar").offsetHeight || 44;
    const spaceAbove = rect.top - titlebarHeight - 8; // Don't overflow past titlebar
    const spaceBelow = window.innerHeight - rect.bottom - 16;
    const popoverHeight = 350;

    if (spaceBelow < popoverHeight && !document.getElementById("chat-container").classList.contains("chat-empty")) {
        modelPopover.classList.add("open-up");
        modelPopover.style.maxHeight = Math.max(200, spaceAbove) + "px";
    } else {
        modelPopover.classList.remove("open-up");
        modelPopover.style.maxHeight = Math.max(200, spaceAbove) + "px";
    }

    modelPopover.classList.toggle("hidden");
});

// Handle Model Selection — fallback for hardcoded HTML models
document.querySelectorAll(".model-option").forEach(opt => {
    opt.addEventListener("click", () => {
        selectModel(opt);
    });
});

// Handle Thinking Level Options
const thinkingToggle = document.getElementById("thinking-toggle");
const thinkingLevelsContainer = document.getElementById("thinking-levels-container");

thinkingToggle.addEventListener("change", (e) => {
    if (e.target.checked) {
        thinkingLevelsContainer.classList.remove("hidden");
        // Ensure there's a valid default selection
        if (currentThinkingLevel === "none") {
            const activeBtn = thinkingLevelsContainer.querySelector(".think-btn.active") || thinkingLevelsContainer.querySelector(".think-btn");
            activeBtn.classList.add("active");
            currentThinkingLevel = activeBtn.getAttribute("data-level");
        }
    } else {
        thinkingLevelsContainer.classList.add("hidden");
        currentThinkingLevel = "none";
    }
});

document.querySelectorAll(".think-btn").forEach(btn => {
    btn.addEventListener("click", (e) => {
        e.stopPropagation(); // Keep popover open
        document.querySelectorAll(".think-btn").forEach(b => b.classList.remove("active"));
        btn.classList.add("active");
        if (thinkingToggle.checked) {
            currentThinkingLevel = btn.getAttribute("data-level");
        }
    });
});

document.addEventListener("click", (e) => {
    if (!modelPopover.contains(e.target) && e.target !== modelBtn) {
        modelPopover.classList.add("hidden");
        // Also collapse More models when closing popover
        const moreModelsSection = document.getElementById("more-models-section");
        const moreModelsBtn = document.getElementById("more-models-btn");
        if (moreModelsSection && moreModelsBtn) {
            moreModelsSection.classList.remove("visible");
            moreModelsSection.classList.add("hidden");
            moreModelsBtn.classList.remove("expanded");
        }
    }
});

// Handle More Models Toggle
document.getElementById("more-models-btn").addEventListener("click", (e) => {
    e.stopPropagation();
    const section = document.getElementById("more-models-section");
    const btn = document.getElementById("more-models-btn");
    const isVisible = section.classList.contains("visible");

    if (isVisible) {
        section.classList.remove("visible");
        section.classList.add("hidden");
        btn.classList.remove("expanded");
    } else {
        section.classList.remove("hidden");
        section.classList.add("visible");
        btn.classList.add("expanded");
    }
});

function getPromptsData() {
    return t("prompts") || {};
}

const suggestedMenuPane = document.getElementById("suggested-menu-pane");
const suggestedMenuList = document.getElementById("suggested-menu-list");
const suggestedMenuTitleText = document.getElementById("suggested-menu-title-text");
const btnCloseSuggestedMenu = document.getElementById("btn-close-suggested-menu");
const suggestedActionsContainer = document.querySelector(".suggested-actions");

// Helper: Shuffle array and pull N items
function getRandomPrompts(array, n) {
    const shuffled = [...array].sort(() => 0.5 - Math.random());
    return shuffled.slice(0, n);
}

// Helper: Word-by-word soft fade-in animation
function softTextEntrance(element, finalString, itemIndex) {
    const words = finalString.split(' ');
    const baseDelay = itemIndex * 0.12; // stagger between menu items
    element.innerHTML = '';
    words.forEach((word, i) => {
        const span = document.createElement('span');
        span.className = 'word-fade';
        span.textContent = word + ' ';
        span.style.animationDelay = `${baseDelay + i * 0.04}s`;
        element.appendChild(span);
    });
}

// Handle Suggested Actions Menu Toggle
document.querySelectorAll(".suggest-btn").forEach(btn => {
    btn.addEventListener("click", () => {
        const preset = btn.getAttribute("data-preset");
        const title = btn.textContent.trim();
        const fetchedPrompts = t(`prompts.${preset}`);
        const basePrompts = Array.isArray(fetchedPrompts) ? fetchedPrompts : [];
        const randomPrompts = getRandomPrompts(basePrompts, 5);

        // SVG Transfer
        const svgIcon = btn.querySelector("svg");
        const headerIconContainer = document.querySelector(".suggested-menu-title svg");
        if (svgIcon && headerIconContainer) {
            headerIconContainer.outerHTML = svgIcon.outerHTML; // Replace completely
            document.querySelector(".suggested-menu-title svg").classList.add("suggested-menu-icon"); // Re-add class
        }

        // Set title
        suggestedMenuTitleText.textContent = title;

        // Populate list
        suggestedMenuList.innerHTML = "";
        randomPrompts.forEach((promptText, index) => {
            const item = document.createElement("div");
            item.className = "suggested-menu-item";
            item.addEventListener("click", () => {
                inputEl.value = promptText;
                inputEl.focus();
                autoResize();
                sendBtn.classList.add("active");
                closeSuggestedMenu();
            });
            suggestedMenuList.appendChild(item);

            // Soft staggered fade-in
            softTextEntrance(item, promptText, index);
        });

        // Toggle visibility with CSS transition trigger
        suggestedActionsContainer.classList.remove("anim-in");
        suggestedActionsContainer.classList.add("anim-out");

        setTimeout(() => {
            const updateUI = () => {
                suggestedActionsContainer.style.display = "none";
                suggestedMenuPane.classList.remove("hidden");
                suggestedMenuPane.classList.remove("anim-out");
                suggestedMenuPane.classList.add("anim-in");
            };

            if (document.startViewTransition) {
                document.startViewTransition(updateUI);
            } else {
                updateUI();
            }
        }, 250);
    });
});

function closeSuggestedMenu() {
    suggestedMenuPane.classList.remove("anim-in");
    suggestedMenuPane.classList.add("anim-out");

    setTimeout(() => {
        const updateUI = () => {
            suggestedMenuPane.classList.add("hidden");
            suggestedActionsContainer.style.display = "flex";
            suggestedActionsContainer.classList.remove("anim-out");
            suggestedActionsContainer.classList.add("anim-in");
        };

        if (document.startViewTransition) {
            document.startViewTransition(updateUI);
        } else {
            updateUI();
        }
    }, 250);
}

btnCloseSuggestedMenu.addEventListener("click", closeSuggestedMenu);

themeBtn.addEventListener("click", toggleTheme);

settingsBtn.addEventListener("click", showModal);

ghostBtn.addEventListener("click", () => {
    // Placeholder for future functionality
    console.log("Ghost button clicked - coming soon!");
});

sendBtn.addEventListener("click", () => {
    if (isProcessing) abortGeneration();
    else sendMessage();
});

inputEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        sendMessage();
    }
});

inputEl.addEventListener("input", () => {
    autoResize();
    updateSendButtonState();
});

saveKeyBtn.addEventListener("click", async () => {
    // 1. Handle Language Save First
    const langSelect = document.getElementById("language-select");
    if (langSelect && langSelect.value !== currentLangCode) {
        const newLang = langSelect.value;
        loadLanguageSync(newLang);
        localStorage.setItem("app_language", newLang);
        applyTranslations();

        // Clear open suggested prompts so they reload in new language when clicked
        suggestedMenuPane.classList.add("hidden");
        suggestedMenuPane.classList.remove("visible");
        document.querySelectorAll(".suggest-btn").forEach(b => b.classList.remove("expanded"));
    }

    // If Google login is active, skip API key validation and just close
    if (isGoogleLoggedIn) {
        hideModal();
        inputEl.focus();
        return;
    }

    const key = apiKeyInput.value.trim();

    // If key is empty or contains masked dots, check if we already have a saved key
    if (!key || key.includes("\u2022")) {
        const result = await ipcRenderer.invoke("load-api-key");
        if (result.key) {
            // Key already saved, just close
            apiKeyInput.placeholder = t("settings.enterNewKey");
            hideModal();
            return;
        } else {
            // No key saved, and no language change just happened? We need a key.
            // If they just changed language but have no key, still force them to enter a key.
            const existing = document.querySelector("#tab-api .error-text");
            if (existing) existing.remove();
            const err = document.createElement("p");
            err.className = "error-text";
            err.textContent = t("settings.enterApiKey");
            document.getElementById("tab-api").appendChild(err);
            return;
        }
    }

    saveKeyBtn.disabled = true;
    saveKeyBtn.textContent = t("settings.saving");

    const existing = document.querySelector("#tab-api .error-text");
    if (existing) existing.remove();

    try {
        const success = await setApiKey(key);
        if (success) {
            await ipcRenderer.invoke("save-api-key", key);
            apiKeyInput.placeholder = "AIza...";
            hideModal();
            await checkStatus();
            await fetchModels();
            inputEl.focus();
        } else {
            const err = document.createElement("p");
            err.className = "error-text";
            err.textContent = t("settings.apiKeyRejected");
            document.getElementById("tab-api").appendChild(err);
        }
    } catch {
        const err = document.createElement("p");
        err.className = "error-text";
        err.textContent = t("settings.cannotConnect");
        document.getElementById("tab-api").appendChild(err);
    }

    saveKeyBtn.disabled = false;
    saveKeyBtn.textContent = t("settings.saveAndClose");
});

toggleKeyBtn.addEventListener("click", () => {
    if (apiKeyInput.type === "password") {
        apiKeyInput.type = "text";
    } else {
        apiKeyInput.type = "password";
    }
});

apiKeyInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
        saveKeyBtn.click();
    }
});

getKeyLink.addEventListener("click", (e) => {
    e.preventDefault();
    shell.openExternal("https://aistudio.google.com/apikey");
});

// --- New Feature Sub-Toggles ---

tempSlider.addEventListener("input", () => {
    tempVal.textContent = tempSlider.value;
});

// --- Attach Popover Menu ---
const attachPopover = document.getElementById("attach-popover");
const attachUploadBtn = document.getElementById("attach-upload-files");
const attachRecordBtn = document.getElementById("attach-record-audio");
const attachCameraBtn = document.getElementById("attach-use-camera");
let mediaRecorder = null;
let audioChunks = [];
let isRecordingAudio = false;

// Toggle popover on attach button click
btnAttach.addEventListener("click", (e) => {
    e.stopPropagation();
    attachPopover.classList.toggle("hidden");
    // Close model popover if open
    modelPopover.classList.add("hidden");
});

// Close popover on outside click
document.addEventListener("click", (e) => {
    if (!attachPopover.contains(e.target) && e.target !== btnAttach && !btnAttach.contains(e.target)) {
        attachPopover.classList.add("hidden");
    }
});

// Option 1: Upload Files
attachUploadBtn.addEventListener("click", () => {
    attachPopover.classList.add("hidden");
    imageUpload.click();
});

// Option 2: Record Audio
// Recording Logic with Floating UI
let recordDurationSec = 0;
let recordTimerInterval = null;

function updateRecordTime() {
    const min = Math.floor(recordDurationSec / 60).toString().padStart(2, "0");
    const sec = (recordDurationSec % 60).toString().padStart(2, "0");
    document.getElementById("record-time").textContent = `${min}:${sec}`;
}

function stopAndCleanupRecording(isCancel) {
    if (isCancel) {
        isRecordingAudio = false;
    }
    if (mediaRecorder && mediaRecorder.state !== "inactive") {
        mediaRecorder.stop(); // This triggers onstop, where attachedFiles is populated
    }
    if (recordTimerInterval) clearInterval(recordTimerInterval);
    document.getElementById("recording-overlay").classList.add("hidden");
    // We let onstop handle throwing away data if isRecordingAudio is false
}

document.getElementById("btn-cancel-record").addEventListener("click", () => {
    stopAndCleanupRecording(true); // Cancel
});

document.getElementById("btn-save-record").addEventListener("click", () => {
    stopAndCleanupRecording(false); // Save
});

attachRecordBtn.addEventListener("click", async () => {
    attachPopover.classList.add("hidden");

    if (mediaRecorder && mediaRecorder.state !== "inactive") return;

    try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

        // Let's use audio/webm as it is widely supported in Electron for recording
        // We will save it as .ogg extension because Gemini likes it, but webm is fine too.
        mediaRecorder = new MediaRecorder(stream, { mimeType: "audio/webm;codecs=opus" });
        audioChunks = [];

        mediaRecorder.ondataavailable = (e) => {
            if (e.data.size > 0) audioChunks.push(e.data);
        };

        // Real-time Audio Visualizer Logic
        const audioContext = new (window.AudioContext || window.webkitAudioContext)();
        const analyser = audioContext.createAnalyser();
        const source = audioContext.createMediaStreamSource(stream);
        source.connect(analyser);
        analyser.fftSize = 64;
        const bufferLength = analyser.frequencyBinCount;
        const dataArray = new Uint8Array(bufferLength);

        const bars = document.querySelectorAll(".waveform .bar");
        let animationFrameId;

        function updateWaveform() {
            if (!isRecordingAudio || mediaRecorder.state === "inactive") return;
            animationFrameId = requestAnimationFrame(updateWaveform);
            analyser.getByteFrequencyData(dataArray);

            // Calculate average volume
            let sum = 0;
            for (let i = 0; i < bufferLength; i++) {
                sum += dataArray[i];
            }
            let avg = sum / bufferLength;

            // Map avg (0-255) to a height between 4px and 22px
            const minHeight = 4;
            const maxHeight = 22;

            // Adjust sensitivity (avg is usually small for normal talking, 0-50)
            // Multiply by a bigger factor to see movement
            let scaled = minHeight + (avg / 60) * (maxHeight - minHeight);

            bars.forEach((bar, j) => {
                // Add tiny variations to each bar based on index for a dynamic look
                let h = scaled * (0.8 + Math.sin((j + 1) * avg * 0.1) * 0.3);

                if (h > maxHeight) h = maxHeight;
                if (h < minHeight || isNaN(h)) h = minHeight;
                bar.style.height = `${h}px`;
            });
        }

        mediaRecorder.onstop = () => {
            stream.getTracks().forEach(track => track.stop());
            if (audioContext.state !== "closed") audioContext.close();
            if (animationFrameId) cancelAnimationFrame(animationFrameId);

            // Reset bars
            bars.forEach(bar => bar.style.height = '4px');

            // If it was cancelled, discard
            if (!isRecordingAudio) {
                audioChunks = [];
                return;
            }

            const blob = new Blob(audioChunks, { type: "audio/webm" });
            if (blob.size > 100 * 1024 * 1024) {
                const sizeMB = (blob.size / (1024 * 1024)).toFixed(1);
                showToast(t("errors.fileTooLarge", { size: sizeMB }), "error");
                return;
            }

            const reader = new FileReader();
            reader.onload = (ev) => {
                const fullDataUrl = ev.target.result;
                const parts = fullDataUrl.split(",");
                const mimeLine = parts[0];
                const mimeType = mimeLine.match(/:(.*?);/)[1];
                const base64 = parts[1];

                // Add to attachments
                attachedFiles.push({ base64, mimeType, name: "Audio_Recording.ogg", dataUrl: fullDataUrl });
                renderAttachmentPreviews();
            };
            reader.readAsDataURL(blob);
            audioChunks = [];
        };

        // Reset and show UI
        recordDurationSec = 0;
        updateRecordTime();
        document.getElementById("recording-overlay").classList.remove("hidden");

        isRecordingAudio = true; // This is crucial for save functionality
        mediaRecorder.start();
        updateWaveform(); // start visualizer

        recordTimerInterval = setInterval(() => {
            recordDurationSec++;
            updateRecordTime();
        }, 1000);

    } catch (err) {
        console.error("Mic access denied or error:", err);
        showToast("Mikrofon erişimi reddedildi veya hata oluştu.", "error");
    }
});

// Option 3: Use Camera (with live preview modal)
attachCameraBtn.addEventListener("click", async () => {
    attachPopover.classList.add("hidden");
    try {
        const stream = await navigator.mediaDevices.getUserMedia({ video: true });

        // Create camera modal overlay
        const overlay = document.createElement("div");
        overlay.id = "camera-modal-overlay";
        overlay.className = "camera-modal-overlay";

        const modal = document.createElement("div");
        modal.className = "camera-modal";

        const video = document.createElement("video");
        video.srcObject = stream;
        video.autoplay = true;
        video.playsInline = true;
        video.className = "camera-preview-video";

        const btnRow = document.createElement("div");
        btnRow.className = "camera-modal-actions";

        const captureBtn = document.createElement("button");
        captureBtn.className = "camera-capture-btn";
        captureBtn.title = t("attach.capturePhoto") || "Capture";

        const closeBtn = document.createElement("button");
        closeBtn.className = "camera-close-btn";
        closeBtn.innerHTML = `
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
        `;

        const cleanupCamera = () => {
            stream.getTracks().forEach(track => track.stop());
            overlay.remove();
        };

        closeBtn.addEventListener("click", (e) => {
            e.stopPropagation();
            cleanupCamera();
        });

        overlay.addEventListener("click", (e) => {
            if (e.target === overlay) cleanupCamera();
        });

        captureBtn.addEventListener("click", () => {
            const canvas = document.createElement("canvas");
            canvas.width = video.videoWidth;
            canvas.height = video.videoHeight;
            canvas.getContext("2d").drawImage(video, 0, 0);
            cleanupCamera();

            const dataUrl = canvas.toDataURL("image/png");
            const parts = dataUrl.split(",");
            const base64 = parts[1];

            attachedFiles.push({ base64, mimeType: "image/png", name: "camera_capture.png", dataUrl });
            renderAttachmentPreviews();
        });

        modal.appendChild(video);
        modal.appendChild(btnRow);
        btnRow.appendChild(captureBtn); // Only capture button in the bottom row now
        overlay.appendChild(modal);
        overlay.appendChild(closeBtn); // Close button is absolute positioned at top right
        document.body.appendChild(overlay);

        await new Promise(resolve => { video.onloadedmetadata = resolve; });
        await video.play();
    } catch (err) {
        console.error("Camera access denied:", err);
        showToast(t("errors.cameraDenied") || "Kamera erişimi reddedildi.", "error");
    }
});


// Check camera availability on load
async function checkCameraAvailability() {
    try {
        const devices = await navigator.mediaDevices.enumerateDevices();
        const hasCamera = devices.some(d => d.kind === "videoinput");
        attachCameraBtn.disabled = !hasCamera;
    } catch {
        attachCameraBtn.disabled = true;
    }
}
checkCameraAvailability();

imageUpload.addEventListener("change", (e) => {
    const files = Array.from(e.target.files);
    if (!files.length) return;

    for (const file of files) {
        // Disallow zip files explicitly
        if (file.name.endsWith(".zip") || file.name.endsWith(".rar") || file.name.endsWith(".7z")) {
            showToast(t("errors.unsupportedFile", { name: file.name }) || `"${file.name}" desteklenmiyor.`, "error");
            continue;
        }

        if (file.size > 100 * 1024 * 1024) {
            const sizeMB = (file.size / (1024 * 1024)).toFixed(1);
            showToast(t("errors.fileTooLarge", { size: sizeMB }), "error");
            continue;
        }

        const reader = new FileReader();
        reader.onload = (ev) => {
            const fullDataUrl = ev.target.result;
            const parts = fullDataUrl.split(",");
            const mimeLine = parts[0];
            const extractedMimeType = mimeLine.match(/:(.*?);/)[1];
            const base64 = parts[1];

            let resolvedMime = getMimeType(file.name, extractedMimeType, file.type);
            if (!resolvedMime || resolvedMime === 'application/octet-stream') {
                resolvedMime = detectMimeFromBase64(base64) || 'application/octet-stream';
            }

            const mimeType = toGeminiMime(resolvedMime);
            if (!mimeType) {
                // Can't be converted — check if truly binary
                if (isLikelyBinary(base64)) {
                    showToast(t("errors.unsupportedFile", { name: file.name }) || `"${file.name}" desteklenmiyor.`, "error");
                    return;
                }
                // Not binary — send as text/plain as last resort
                const safeUrl = `data:text/plain;base64,${base64}`;
                attachedFiles.push({ base64, mimeType: 'text/plain', name: file.name, dataUrl: safeUrl });
                renderAttachmentPreviews();
                return;
            }

            const correctedDataUrl = resolvedMime !== mimeType
                ? `data:${mimeType};base64,${base64}`
                : fullDataUrl;

            attachedFiles.push({ base64, mimeType, name: file.name, dataUrl: correctedDataUrl });
            renderAttachmentPreviews();
        };
        reader.readAsDataURL(file);
    }
});

function renderAttachmentPreviews() {
    // Hide single legacy preview
    previewImg.classList.add("hidden");
    const oldIcon = document.getElementById("generic-file-icon");
    if (oldIcon) oldIcon.classList.add("hidden");

    // Get or create multi-preview container
    let container = document.getElementById("multi-preview-container");
    if (!container) {
        container = document.createElement("div");
        container.id = "multi-preview-container";
        container.className = "multi-preview-strip";
        imagePreview.insertBefore(container, btnRemoveImg);
    }
    container.innerHTML = "";

    attachedFiles.forEach((f, index) => {
        const item = document.createElement("div");
        item.className = "multi-preview-item";

        // Add click to preview
        item.addEventListener("click", () => {
            if (f.mimeType.startsWith("image/")) {
                openImageLightbox(f.dataUrl);
            }
        });

        // Add remove button
        const removeBtn = document.createElement("div");
        removeBtn.className = "remove-btn";
        removeBtn.innerHTML = `
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
        `;

        removeBtn.addEventListener("click", (e) => {
            e.stopPropagation(); // Don't trigger preview
            attachedFiles.splice(index, 1);
            renderAttachmentPreviews();
        });
        item.appendChild(removeBtn);

        if (f.mimeType.startsWith("image/")) {
            const img = document.createElement("img");
            img.src = f.dataUrl;
            img.alt = f.name;
            item.appendChild(img);
        } else if (f.mimeType.startsWith("audio/")) {
            const customPlayer = createCustomAudioPlayer(f.dataUrl);
            item.appendChild(customPlayer);
            // Don't trigger standard preview on click for audio controls
            item.onclick = (e) => e.stopPropagation();
        } else {
            const ext = f.name.split('.').pop().substring(0, 4);
            const fileIcon = document.createElement("div");
            fileIcon.style.display = "contents";
            fileIcon.innerHTML = `
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                    <polyline points="14 2 14 8 20 8"></polyline>
                </svg>
                <span class="multi-preview-ext">${ext.toUpperCase()}</span>
            `;
            item.appendChild(fileIcon);
        }
        container.appendChild(item);
    });

    if (attachedFiles.length > 0) {
        imagePreview.classList.remove("hidden");
        btnAttach.classList.add("active");
    } else {
        imagePreview.classList.add("hidden");
        btnAttach.classList.remove("active");
    }
    updateSendButtonState();
}


// Make the input preview image clickable
previewImg.addEventListener("click", () => {
    if (previewImg.src) {
        openImageLightbox(previewImg.src);
    }
});
previewImg.style.cursor = "pointer";

document.body.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.stopPropagation();
    document.body.classList.add("drag-active");
});

document.body.addEventListener("dragleave", (e) => {
    e.preventDefault();
    e.stopPropagation();
    // Only remove if we really left the window
    if (!e.relatedTarget || e.relatedTarget.nodeName === "HTML") {
        document.body.classList.remove("drag-active");
    }
});

document.body.addEventListener("drop", (e) => {
    e.preventDefault();
    e.stopPropagation();
    document.body.classList.remove("drag-active");

    const files = Array.from(e.dataTransfer.files);
    if (!files.length) return;

    for (const file of files) {
        if (file.name.endsWith(".zip") || file.name.endsWith(".rar") || file.name.endsWith(".7z")) {
            showToast(t("errors.unsupportedFile", { name: file.name }) || `"${file.name}" desteklenmiyor.`, "error");
            continue;
        }
        if (file.size > 100 * 1024 * 1024) {
            const sizeMB = (file.size / (1024 * 1024)).toFixed(1);
            showToast(t("errors.fileTooLarge", { size: sizeMB }), "error");
            continue;
        }
        const reader = new FileReader();
        reader.onload = (ev) => {
            const fullDataUrl = ev.target.result;
            const parts = fullDataUrl.split(",");
            const mimeLine = parts[0];
            const base64 = parts[1];

            let resolvedMime = getMimeType(file.name, mimeLine.match(/:(.*?);/)[1], file.type);
            if (!resolvedMime || resolvedMime === 'application/octet-stream') {
                resolvedMime = detectMimeFromBase64(base64) || 'application/octet-stream';
            }

            const mimeType = toGeminiMime(resolvedMime);
            if (!mimeType) {
                if (isLikelyBinary(base64)) {
                    showToast(t("errors.unsupportedFile", { name: file.name }) || `"${file.name}" desteklenmiyor.`, "error");
                    return;
                }
                attachedFiles.push({ base64, mimeType: 'text/plain', name: file.name, dataUrl: `data:text/plain;base64,${base64}` });
                renderAttachmentPreviews();
                return;
            }

            const correctedDataUrl = resolvedMime !== mimeType
                ? `data:${mimeType};base64,${base64}`
                : fullDataUrl;
            attachedFiles.push({ base64, mimeType, name: file.name, dataUrl: correctedDataUrl });
            renderAttachmentPreviews();
        };
        reader.readAsDataURL(file);
    }
});

// Ctrl+V paste file support
document.addEventListener("paste", (e) => {
    const items = e.clipboardData?.items;
    if (!items) return;

    for (const item of items) {
        if (item.kind === "file") {
            e.preventDefault();
            const file = item.getAsFile();
            if (!file) continue;
            if (file.name.endsWith(".zip") || file.name.endsWith(".rar") || file.name.endsWith(".7z")) {
                showToast(t("errors.unsupportedFile", { name: file.name }) || `"${file.name}" desteklenmiyor.`, "error");
                continue;
            }
            if (file.size > 100 * 1024 * 1024) {
                const sizeMB = (file.size / (1024 * 1024)).toFixed(1);
                showToast(t("errors.fileTooLarge", { size: sizeMB }), "error");
                continue;
            }
            const reader = new FileReader();
            reader.onload = (ev) => {
                const fullDataUrl = ev.target.result;
                const parts = fullDataUrl.split(",");
                const mimeLine = parts[0];
                const base64 = parts[1];

                let resolvedMime = getMimeType(file.name, mimeLine.match(/:(.*?);/)[1], file.type);
                if (!resolvedMime || resolvedMime === 'application/octet-stream') {
                    resolvedMime = detectMimeFromBase64(base64) || 'application/octet-stream';
                }

                const mimeType = toGeminiMime(resolvedMime);
                if (!mimeType) {
                    if (isLikelyBinary(base64)) {
                        showToast(t("errors.unsupportedFile", { name: file.name }) || `"${file.name}" desteklenmiyor.`, "error");
                        return;
                    }
                    attachedFiles.push({ base64, mimeType: 'text/plain', name: file.name, dataUrl: `data:text/plain;base64,${base64}` });
                    renderAttachmentPreviews();
                    return;
                }

                const correctedDataUrl = resolvedMime !== mimeType
                    ? `data:${mimeType};base64,${base64}`
                    : fullDataUrl;
                attachedFiles.push({ base64, mimeType, name: file.name, dataUrl: correctedDataUrl });
                renderAttachmentPreviews();
            };
            reader.readAsDataURL(file);
        }
    }
});

// Escape key to close lightbox
document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
        const lightbox = document.getElementById("image-lightbox");
        if (lightbox && lightbox.classList.contains("active")) {
            lightbox.classList.remove("active");
        }
    }
});

document.addEventListener("click", () => {
    document.querySelectorAll('.sidebar-item-menu').forEach(m => m.classList.add('hidden'));
});

btnRemoveImg.addEventListener("click", () => {
    attachedFiles = [];
    imagePreview.classList.add("hidden");
    previewImg.src = "";
    previewImg.classList.remove("hidden");
    imageUpload.value = "";
    btnAttach.classList.remove("active");
    const fileIcon = document.getElementById("generic-file-icon");
    if (fileIcon) fileIcon.classList.add("hidden");
    const multiContainer = document.getElementById("multi-preview-container");
    if (multiContainer) multiContainer.innerHTML = "";
});

btnSearch.addEventListener("click", () => {
    useSearch = !useSearch;
    if (useSearch) {
        btnSearch.classList.add("active");
    } else {
        btnSearch.classList.remove("active");
    }
});

btnCode.addEventListener("click", () => {
    useCode = !useCode;
    if (useCode) {
        btnCode.classList.add("active");
    } else {
        btnCode.classList.remove("active");
    }
});

if (btnSidebarToggle) {
    btnSidebarToggle.addEventListener("click", () => {
        sidebar.classList.toggle("sidebar-closed");
    });
}
if (btnNewChat) {
    btnNewChat.addEventListener("click", () => {
        startNewSession();
        if (window.innerWidth < 768) {
            sidebar.classList.add("sidebar-closed"); // close on mobile
        }
    });
}
if (btnOpenSearch) {
    btnOpenSearch.addEventListener("click", () => {
        searchView.classList.remove("hidden");
        activeSearchInput.focus();
        const q = getSearchQuery();
        if (q) {
            triggerSearch(q);
        }
    });
}
if (btnCloseSearch) {
    btnCloseSearch.addEventListener("click", () => {
        searchView.classList.add("hidden");
    });
}
if (searchView) {
    document.addEventListener("keydown", (e) => {
        if (e.key === "Escape" && !searchView.classList.contains("hidden")) {
            closeSearchView();
        } else if (e.key === "Escape" && !artifactSidebar.classList.contains("hidden")) {
            // Optional: prevent closing artifact via escape if it's blocking?
            // closeArtifactSidebar();
        }
    });

    // Handle "Escape" separately for suggested menu
    document.addEventListener("keydown", (e) => {
        if (e.key === "Escape" && !suggestedMenuPane.classList.contains("hidden")) {
            closeSuggestedMenu();
        }
    });
}

// ─── ARTIFACT SIDEBAR LOGIC ─────────────────────────────────────────

let originalPlanLines = [];

function openArtifactSidebar(markdownText, chatId) {
    activeArtifactTx = chatId;
    currentArtifactComments = {};

    // Attempt to split original text somewhat predictably for the final submission
    originalPlanLines = markdownText.split(/(?=\n#{1,6}\s|\n- |\n[0-9]+\.\s)/).map(s => s.trim()).filter(s => s);
    if (originalPlanLines.length === 0) {
        originalPlanLines = [markdownText]; // Fallback
    }

    // Render HTML
    const html = marked.parse(markdownText);

    // Create a temporary container to manipulate the DOM
    const tempDiv = document.createElement("div");
    tempDiv.innerHTML = html;

    // Convert top-level block elements into interactive blocks
    const children = Array.from(tempDiv.children);
    tempDiv.innerHTML = "";

    children.forEach((child, index) => {
        // Only wrap substantive elements (exclude headers so they can't be commented on)
        if (["P", "UL", "OL", "PRE", "BLOCKQUOTE", "TABLE"].includes(child.tagName)) {
            const wrapper = document.createElement("div");
            wrapper.className = "artifact-block";
            wrapper.dataset.blockIndex = index;

            const contentDiv = document.createElement("div");
            contentDiv.className = "artifact-block-content";
            contentDiv.appendChild(child);
            wrapper.appendChild(contentDiv);

            const triggerBtn = document.createElement("button");
            triggerBtn.className = "artifact-comment-trigger";
            triggerBtn.title = "Add a comment to this section";
            triggerBtn.innerHTML = `
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"></path>
                </svg>
            `;

            triggerBtn.addEventListener("click", () => showCommentBox(wrapper, index));
            wrapper.appendChild(triggerBtn);

            tempDiv.appendChild(wrapper);
        } else {
            tempDiv.appendChild(child);
        }
    });

    artifactContentArea.innerHTML = "";
    artifactContentArea.appendChild(tempDiv);

    // Show UI
    artifactSidebar.classList.remove("hidden");
    const centerColumn = document.getElementById("center-column");
    if (centerColumn) centerColumn.style.marginRight = "0"; // Reset any old hardcoded margins

    sidebar.classList.remove("sidebar-open");
    sidebar.classList.add("sidebar-closed");
}

function showCommentBox(blockEl, blockIndex) {
    // Check if already exists
    let existingBox = blockEl.querySelector('.artifact-comment-box');
    if (existingBox) {
        existingBox.querySelector('textarea').focus();
        return;
    }

    const trigger = blockEl.querySelector('.artifact-comment-trigger');
    trigger.classList.add('active');

    const box = document.createElement("div");
    box.className = "artifact-comment-box";

    const textarea = document.createElement("textarea");
    textarea.placeholder = "Write your feedback specifically about this part...";
    if (currentArtifactComments[blockIndex]) {
        textarea.value = currentArtifactComments[blockIndex];
    }
    box.appendChild(textarea);

    const actionsRow = document.createElement("div");
    actionsRow.className = "artifact-comment-actions";

    const btnCancel = document.createElement("button");
    btnCancel.className = "btn-secondary";
    btnCancel.textContent = "Cancel";
    btnCancel.onclick = () => {
        box.remove();
        if (!currentArtifactComments[blockIndex]) {
            trigger.classList.remove('active');
        } else {
            updateCommentBadge(blockEl, true);
        }
    };

    const btnSave = document.createElement("button");
    btnSave.className = "btn-primary";
    btnSave.textContent = "Save";
    btnSave.onclick = () => {
        const val = textarea.value.trim();
        if (val) {
            currentArtifactComments[blockIndex] = val;
            updateCommentBadge(blockEl, true);
        } else {
            delete currentArtifactComments[blockIndex];
            trigger.classList.remove('active');
            updateCommentBadge(blockEl, false);
        }
        box.remove();
        updateProceedButtonState();
    };

    actionsRow.appendChild(btnCancel);
    actionsRow.appendChild(btnSave);
    box.appendChild(actionsRow);

    blockEl.appendChild(box);
    textarea.focus();
}

function updateCommentBadge(blockEl, hasComment) {
    let badge = blockEl.querySelector('.artifact-comment-badge');
    const trigger = blockEl.querySelector('.artifact-comment-trigger');
    if (hasComment) {
        if (!badge) {
            badge = document.createElement("div");
            badge.className = "artifact-comment-badge";
            badge.textContent = "1";
            trigger.appendChild(badge);
        }
        trigger.classList.add('active');
    } else {
        if (badge) badge.remove();
        trigger.classList.remove('active');
    }
}

function updateProceedButtonState() {
    const commentCount = Object.keys(currentArtifactComments).length;
    if (commentCount > 0) {
        btnArtifactProceed.textContent = t("artifact.submit", { count: commentCount }) || `Submit (${commentCount})`;
    } else {
        btnArtifactProceed.textContent = t("artifact.proceed") || "Proceed";
    }
}

function closeArtifactSidebar() {
    artifactSidebar.classList.add("hidden");
    const centerColumn = document.getElementById("center-column");
    if (centerColumn) centerColumn.style.marginRight = "0";

    // If the sidebar is being closed while a plan is active (e.g. Abort button pressed)
    if (activeArtifactTx && isAborting) {
        fetch(`${API_BASE}/chat/plan_response`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: activeArtifactTx, response: "User aborted the generation request." })
        }).catch(e => console.error("Failed to abort plan:", e));
    }

    activeArtifactTx = null;
    currentArtifactComments = {};
    updateProceedButtonState();
}

btnArtifactProceed.addEventListener("click", async () => {
    if (!activeArtifactTx) return;

    const commentCount = Object.keys(currentArtifactComments).length;
    let finalResponse = "";

    if (commentCount === 0) {
        finalResponse = "Approved.";
    } else {
        // Construct detailed feedback mapping
        finalResponse = "User provided the following feedback on specific parts of your plan:\n\n";

        const blocks = document.querySelectorAll('.artifact-block');
        blocks.forEach((block) => {
            const index = block.dataset.blockIndex;
            const comment = currentArtifactComments[index];
            if (comment) {
                // Extract plain text content of this block
                const sectionContent = block.querySelector('.artifact-block-content').innerText.trim().substring(0, 150) + "...";
                finalResponse += `**Regarding section:** "${sectionContent}"\n`;
                finalResponse += `> **Comment:** ${comment}\n\n`;
            }
        });
        finalResponse += "Please revise your plan or execution based on this feedback immediately.";
    }

    const btnOrigText = btnArtifactProceed.textContent;
    btnArtifactProceed.textContent = t("artifact.sending") || "Sending...";
    btnArtifactProceed.disabled = true;

    try {
        await fetch(`${API_BASE}/chat/plan_response`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: activeArtifactTx, response: finalResponse })
        });
    } catch (e) {
        console.error("Failed to submit plan response:", e);
        showToast("Failed to submit review", "error");
    } finally {
        btnArtifactProceed.textContent = btnOrigText;
        btnArtifactProceed.disabled = false;
        closeArtifactSidebar();
    }
});
function getSearchQuery() {
    let q = activeSearchInput ? activeSearchInput.value.trim() : "";
    const chips = document.querySelectorAll(".filter-chip.active");
    chips.forEach(chip => {
        q += " " + chip.dataset.filter;
    });
    return q.trim();
}

const btnSearchFilters = document.getElementById("btn-search-filters");
const searchFiltersDropdown = document.getElementById("search-filters-dropdown");

if (btnSearchFilters && searchFiltersDropdown) {
    btnSearchFilters.addEventListener("click", (e) => {
        e.stopPropagation();
        searchFiltersDropdown.classList.toggle("hidden");
    });

    document.addEventListener("click", (e) => {
        if (!searchFiltersDropdown.contains(e.target) && e.target !== btnSearchFilters) {
            searchFiltersDropdown.classList.add("hidden");
        }
    });
}

document.querySelectorAll(".filter-chip").forEach(chip => {
    chip.addEventListener("click", (e) => {
        e.stopPropagation(); // keep dropdown open when clicking a filter
        chip.classList.toggle("active");
        const q = getSearchQuery();
        if (q) triggerSearch(q);
        else searchViewResults.innerHTML = '';
    });
});

if (activeSearchInput) {
    activeSearchInput.addEventListener("input", () => {
        clearTimeout(searchDebounce);
        searchDebounce = setTimeout(() => {
            const q = getSearchQuery();
            if (q) triggerSearch(q);
            else searchViewResults.innerHTML = '';
        }, 300);
    });
}

async function triggerSearch(query) {
    if (!query) {
        searchViewResults.innerHTML = '';
        return;
    }
    try {
        const response = await fetch(`${API_BASE}/history/search?q=${encodeURIComponent(query)}`);
        const results = await response.json();

        searchViewResults.innerHTML = '';
        if (results.length === 0) {
            searchViewResults.innerHTML = '<div style="text-align:center; color: var(--text-tertiary); margin-top: 24px;">No matching chats found.</div>';
            return;
        }

        results.forEach(chat => {
            const item = document.createElement("div");
            item.className = "search-result-item";

            const title = document.createElement("div");
            title.className = "search-result-title";
            title.textContent = chat.title;

            const date = document.createElement("div");
            date.className = "search-result-date";
            date.textContent = new Date(chat.updated_at).toLocaleString();

            item.appendChild(title);
            if (chat.snippet) {
                const snippet = document.createElement("div");
                snippet.className = "search-result-snippet";

                let html = chat.snippet;
                const terms = query.split(/\s+/).filter(v => !v.includes(":"));
                terms.forEach(term => {
                    if (term) {
                        const regex = new RegExp(`(${term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, "gi");
                        html = html.replace(regex, '<mark style="background: var(--accent); color: var(--accent-fg); padding: 0 2px; border-radius: 2px;">$1</mark>');
                    }
                });

                snippet.innerHTML = html;
                item.appendChild(snippet);
            }
            item.appendChild(date);

            item.addEventListener("click", async () => {
                searchView.classList.add("hidden");
                if (window.innerWidth < 768) {
                    sidebar.classList.add("sidebar-closed");
                }
                await loadSession(chat.id);
            });

            searchViewResults.appendChild(item);
        });
    } catch (e) {
        console.error("Search failed:", e);
    }
}
if (btnClearHistory) {
    btnClearHistory.addEventListener("click", async () => {
        const confirmed = await showConfirm(
            t("settings.clearHistory", { default: "Clear All History" }),
            t("sidebar.confirmClearAll", { default: "Are you sure you want to delete ALL chat history? This cannot be undone." })
        );

        if (confirmed) {
            try {
                await fetch(`${API_BASE}/history/clear_all`, { method: "POST" });
                startNewSession(); // clear current session
                hideModal(); // close settings modal
                // Try sending an event to clear up the actual UI messages div if we are currently looking at one
                document.getElementById("messages").innerHTML = "";
            } catch (e) {
                console.error("Failed to clear history", e);
            }
        }
    });
}

async function init() {
    initLanguage();
    initTheme();
    await loadUserAvatar();
    loadSidebarHistory();

    // Try to restore Google login first
    await restoreGoogleLogin();

    if (!isGoogleLoggedIn) {
        // Try to restore saved API key from encrypted storage
        try {
            const result = await ipcRenderer.invoke("load-api-key");
            if (result.key) {
                await setApiKey(result.key);
            }
        } catch (err) {
            console.error("Failed to load saved API key:", err);
        }

        // Migrate from localStorage if exists (one-time)
        const oldKey = localStorage.getItem("gemini_api_key");
        if (oldKey) {
            await setApiKey(oldKey);
            await ipcRenderer.invoke("save-api-key", oldKey);
            localStorage.removeItem("gemini_api_key");
        }
    }

    const hasKey = await checkStatus();
    if (!hasKey) {
        showModal();
    } else {
        inputEl.focus();
    }
    await fetchModels();

    setInterval(checkStatus, 30000);
}

init();
