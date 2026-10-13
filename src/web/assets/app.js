"use strict";
const byId = (id) => document.getElementById(id);
let token = "";
let polling = false;
let logsSnapshot = [];
let commandsSnapshot = [];
let editingCommandId = null;
let maxMediaBytes = 64 * 1024 * 1024;

const views = {
  connection: ["Overview", "Current bot connection and activity."],
  logs: [
    "Message activity",
    "Messages, commands, and system events in one feed.",
  ],
  send: ["Send & queue", "Send now or schedule your messages."],
  commands: ["Commands", "Available commands and their access rules."],
  settings: ["Settings", "Manage your bot's identity, language, and access."],
};

function navigate(focus = false) {
  const view = Object.hasOwn(views, location.hash.slice(1))
    ? location.hash.slice(1)
    : "connection";
  document.querySelectorAll(".view").forEach((panel) => {
    panel.hidden = panel.id !== view;
  });
  document.querySelectorAll("#navigation a").forEach((link) => {
    if (link.hash === `#${view}`) link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  });
  [byId("page-title").textContent, byId("page-description").textContent] =
    views[view];
  byId("nav-toggle").setAttribute("aria-expanded", "false");
  document.querySelector(".sidebar").dataset.open = "false";
  if (focus && token) byId("page-title").focus();
}
window.addEventListener("hashchange", () => navigate(true));
byId("navigation").addEventListener("click", (event) => {
  const link = event.target.closest("a");
  if (link && link.hash === location.hash) navigate(true);
});
byId("nav-toggle").onclick = () => {
  const open = byId("nav-toggle").getAttribute("aria-expanded") !== "true";
  byId("nav-toggle").setAttribute("aria-expanded", String(open));
  document.querySelector(".sidebar").dataset.open = String(open);
};
document.addEventListener("keydown", (event) => {
  if (
    event.key === "Escape" &&
    byId("nav-toggle").getAttribute("aria-expanded") === "true"
  ) {
    navigate();
    byId("nav-toggle").focus();
  }
});

function setTheme(theme) {
  document.documentElement.dataset.theme = theme;
  byId("theme").textContent = theme === "dark" ? "Light theme" : "Dark theme";
  byId("theme").setAttribute("aria-pressed", String(theme === "light"));
}
try {
  setTheme(
    localStorage.getItem("dashboard-theme") === "light" ? "light" : "dark",
  );
} catch {
  setTheme("dark");
}
byId("theme").onclick = () => {
  const theme =
    document.documentElement.dataset.theme === "dark" ? "light" : "dark";
  setTheme(theme);
  try {
    localStorage.setItem("dashboard-theme", theme);
  } catch {
    /* Private browsing can block storage. */
  }
};

function element(tag, text, className) {
  const node = document.createElement(tag);
  node.textContent = text;
  if (className) node.className = className;
  return node;
}
function badge(text, tone) {
  const node = element("span", text, "badge");
  node.dataset.tone = tone;
  return node;
}
function renderLogs() {
  const query = byId("log-search").value.toLocaleLowerCase();
  const kind = byId("log-kind").value;
  const scope = byId("log-scope").value;
  const matches = logsSnapshot
    .filter((entry) => {
      const details = entry.details || {};
      return (
        (kind === "all" || (details.kind || "system") === kind) &&
        (scope === "all" || details.chat_type === scope) &&
        [entry.message, ...Object.values(details)]
          .join(" ")
          .toLocaleLowerCase()
          .includes(query)
      );
    })
    .reverse();
  byId("log-count").textContent =
    `${matches.length} of ${logsSnapshot.length} entries · Local time ${Intl.DateTimeFormat().resolvedOptions().timeZone}${byId("pause-logs").checked ? " · Feed paused" : " · Newest first"}`;
  const feed = byId("log-content");
  feed.replaceChildren();
  if (!matches.length) {
    feed.append(
      element(
        "p",
        logsSnapshot.length
          ? "No activity matches your filters."
          : "No activity yet. New messages will appear here.",
        "empty",
      ),
    );
    return;
  }
  for (const entry of matches) {
    const details = entry.details || {};
    const item = element("article", "", "log-entry");
    item.dataset.kind = details.kind || "system";
    item.dataset.scope = details.chat_type || "system";
    item.dataset.level = entry.level;
    const header = element("div", "", "log-header");
    header.append(
      badge(
        details.kind === "command"
          ? "COMMAND"
          : details.kind === "message"
            ? "MESSAGE"
            : entry.level,
        details.kind ||
          (entry.level === "ERROR" || entry.level === "WARN"
            ? "error"
            : "system"),
      ),
    );
    if (details.chat_type)
      header.append(
        badge(
          details.chat_type === "group" ? "GROUP" : "PRIVATE",
          details.chat_type,
        ),
      );
    if (details.direction)
      header.append(
        element(
          "span",
          details.direction === "incoming" ? "Incoming" : "Outgoing",
        ),
      );
    const time = element(
      "time",
      new Date(entry.time).toLocaleString("en-US", {
        dateStyle: "medium",
        timeStyle: "medium",
      }),
    );
    time.dateTime = entry.time;
    header.append(time);
    item.append(header);
    if (details.sender)
      item.append(element("p", details.name || details.sender, "log-sender"));
    item.append(
      element(
        "p",
        details.preview ||
          (details.media ? `[${details.media}]` : entry.message),
        "log-preview",
      ),
    );
    if (details.chat)
      item.append(
        element(
          "p",
          `${details.sender} → ${details.chat} · ${details.media}${details.command ? ` · /${details.command}` : ""}`,
          "log-meta",
        ),
      );
    feed.append(item);
  }
}
for (const id of ["log-search", "log-kind", "log-scope"])
  byId(id).addEventListener("input", renderLogs);
byId("pause-logs").onchange = () => {
  renderLogs();
  if (!byId("pause-logs").checked)
    refresh().catch((error) => notice(error.message, true));
};

function renderCommands() {
  const query = byId("command-search").value.toLocaleLowerCase();
  const commands = commandsSnapshot.filter((c) =>
    [c.name, ...c.aliases, c.category, c.description]
      .join(" ")
      .toLocaleLowerCase()
      .includes(query),
  );
  renderRows(
    "commands-body",
    commands.map((c) => {
      const controls = element("div", "", "row-actions");
      const edit = element("button", "Edit", "secondary");
      edit.type = "button";
      edit.id = `edit-command-${c.id}`;
      edit.setAttribute("aria-label", `Edit ${c.name}`);
      edit.onclick = () => openCommandEditor(c);
      const remove = element("button", "Delete", "secondary danger");
      remove.type = "button";
      remove.id = `delete-command-${c.id}`;
      remove.setAttribute("aria-label", `Delete ${c.name}`);
      remove.onclick = () => {
        if (
          !confirm(
            `Delete command "${c.name}" and its aliases? It will no longer run or appear in the bot menu.`,
          )
        ) return;
        action(remove, async () => {
          await api(`commands/${encodeURIComponent(c.id)}`, "DELETE");
          commandsSnapshot = await api("commands");
          renderCommands();
          if (editingCommandId === c.id) closeCommandEditor();
          byId("command-search").focus();
          notice(`Command "${c.name}" deleted.`);
        });
      };
      controls.append(edit, remove);
      return [
        c.name + (c.aliases.length ? ` / ${c.aliases.join(", ")}` : ""),
        c.description,
        c.category,
        `${c.cooldown_secs} sec`,
        c.limit_cost,
        c.scope,
        c.owner_only ? "Owners" : "Users",
        controls,
      ];
    }),
    commandsSnapshot.length ? "No commands match your search." : "No active commands.",
    8,
  );
}
byId("command-search").oninput = renderCommands;

function openCommandEditor(command) {
  editingCommandId = command.id;
  byId("command-editor-title").textContent = `Edit command: ${command.name}`;
  byId("command-name").value = command.name;
  byId("command-aliases").value = command.aliases.join(", ");
  byId("command-category").value = command.category;
  byId("command-description").value = command.description_override || "";
  byId("command-description").placeholder = command.description;
  byId("command-cooldown").value = command.cooldown_secs;
  byId("command-cost").value = command.limit_cost;
  byId("command-scope").value = command.scope;
  byId("command-owner").checked = command.owner_only;
  byId("command-owner").disabled = command.owner_access_required;
  byId("command-editor").hidden = false;
  byId("command-name").focus();
}

function closeCommandEditor() {
  const id = editingCommandId;
  editingCommandId = null;
  byId("command-editor").hidden = true;
  byId("command-form").reset();
  (byId(`edit-command-${id}`) || byId("command-search")).focus();
}

byId("cancel-command").onclick = closeCommandEditor;
byId("command-form").addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    event.preventDefault();
    closeCommandEditor();
  }
});
byId("command-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const id = editingCommandId;
  if (!id) return;
  action(event.submitter, async () => {
    await api(`commands/${encodeURIComponent(id)}`, "PUT", {
      name: byId("command-name").value.trim(),
      aliases: byId("command-aliases")
        .value.split(",")
        .map((s) => s.trim())
        .filter(Boolean),
      category: byId("command-category").value.trim(),
      description: byId("command-description").value.trim() || null,
      cooldown_secs: Number(byId("command-cooldown").value),
      limit_cost: Number(byId("command-cost").value),
      scope: byId("command-scope").value,
      owner_only: byId("command-owner").checked,
    });
    commandsSnapshot = await api("commands");
    renderCommands();
    closeCommandEditor();
    notice("Command saved and applied.");
  });
});

function notice(message, failed = false) {
  byId("notice").textContent = message;
  byId("notice").dataset.error = String(failed);
}

async function api(path, method = "GET", payload) {
  const requestToken = token;
  const response = await fetch(`/api/${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${requestToken}`,
      "Content-Type": "application/json",
    },
    body: payload === undefined ? undefined : JSON.stringify(payload),
    signal: AbortSignal.timeout(65000),
  });
  const text = await response.text();
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error(
      `The server rejected the request (${response.status}). Check the file size and connection.`,
    );
  }
  if (!response.ok) {
    if (response.status === 401 && token && token === requestToken) logout();
    throw new Error(data.error || `Request failed (${response.status})`);
  }
  return data;
}

async function action(button, callback) {
  const hadFocus = document.activeElement === button;
  button.disabled = true;
  notice("Working...");
  try {
    await callback();
  } catch (error) {
    notice(error.message, true);
  } finally {
    button.disabled = false;
    if (
      hadFocus &&
      document.activeElement === document.body &&
      button.isConnected
    )
      button.focus();
  }
}

function logout() {
  token = "";
  logsSnapshot = [];
  commandsSnapshot = [];
  byId("workspace").hidden = true;
  document.querySelector(".skip-link").href = "#login-title";
  byId("login-panel").hidden = false;
  byId("qr").removeAttribute("src");
  byId("qr").hidden = true;
  for (const id of [
    "metrics",
    "runtime",
    "jobs-body",
    "commands-body",
    "log-content",
  ])
    byId(id).replaceChildren();
  byId("pause-logs").checked = false;
  byId("log-search").value = "";
  byId("log-kind").value = "all";
  byId("log-scope").value = "all";
  byId("command-search").value = "";
  byId("connection-state").textContent = "Loading status...";
  byId("settings-form").reset();
  byId("sticker-form").reset();
  byId("command-form").reset();
  byId("command-editor").hidden = true;
  editingCommandId = null;
  byId("send-form").reset();
  updateKind();
  notice("Signed out of dashboard.");
  byId("token").focus();
}

function renderRows(target, rows, empty, columns) {
  const body = byId(target);
  body.replaceChildren();
  if (rows.length === 0) {
    const row = body.insertRow();
    const cell = row.insertCell();
    cell.colSpan = columns;
    cell.textContent = empty;
    return;
  }
  for (const values of rows) {
    const row = body.insertRow();
    for (const value of values) {
      const cell = row.insertCell();
      if (value instanceof Node) cell.append(value);
      else cell.textContent = String(value);
    }
  }
}

async function refresh() {
  if (!token || polling) return;
  polling = true;
  const requestToken = token;
  try {
    const [status, jobs, logs, settings, commands] = await Promise.all([
      api("status"),
      api("messages"),
      api("logs"),
      api("settings"),
      api("commands"),
    ]);
    if (!token || token !== requestToken) return;
    if (JSON.stringify(commandsSnapshot) !== JSON.stringify(commands)) {
      const focusedCommandButton = document.activeElement.closest("#commands-body")
        ? document.activeElement.id
        : null;
      commandsSnapshot = commands;
      renderCommands();
      if (focusedCommandButton)
        (byId(focusedCommandButton) || byId("command-search")).focus({
          preventScroll: true,
        });
    }
    const labels = {
      offline: "Offline",
      starting: "Connecting",
      connected: "Connected",
      pairing: "Waiting for QR scan",
      reconnecting: "Reconnecting",
      logged_out: "Logged out",
      client_outdated: "Library update required",
      pairing_expired: "Pairing expired",
      stopped: "Connection stopped",
    };
    byId("connection-state").textContent =
      labels[status.connection.state] || status.connection.state;
    byId("connection-state").dataset.state = status.connection.state;
    byId("connection-help").textContent =
      status.connection.state === "pairing"
        ? "On your phone: WhatsApp > Linked devices > Link a device. Scan the QR code before it expires."
        : [
              "logged_out",
              "pairing_expired",
              "stopped",
              "client_outdated",
            ].includes(status.connection.state)
          ? "Restart the bot process. If the client is outdated, update the whatsapp-rust dependency first."
          : status.connection.state === "offline"
            ? "Offline mode is active. Queued messages will wait until the bot connects to WhatsApp."
            : status.connection.state === "connected"
              ? "Device linked. The bot is ready to receive messages and run commands according to its access mode."
              : "Waiting for WhatsApp. Status refreshes automatically.";
    const qr = byId("qr");
    qr.hidden = !status.connection.qr_data_url;
    if (status.connection.qr_data_url) qr.src = status.connection.qr_data_url;
    else qr.removeAttribute("src");
    const metrics = byId("metrics");
    metrics.replaceChildren();
    for (const [label, value] of [
      ["Messages received", status.received],
      ["Commands executed", status.commands],
      ["Messages sent", status.sent],
      ["Failures", status.failures],
    ]) {
      const item = document.createElement("div");
      const term = document.createElement("dt");
      const detail = document.createElement("dd");
      term.textContent = label;
      detail.textContent = value;
      item.append(term, detail);
      metrics.append(item);
    }
    byId("runtime").replaceChildren();
    for (const [label, value] of [
      ["Bot name", settings.name],
      ["Mode / prefix", `${settings.mode} / ${settings.prefix}`],
      ["Reply language", settings.language === "id" ? "Indonesian" : "English"],
      [
        "Uptime",
        `${Math.floor(status.uptime_secs / 3600)}h ${Math.floor(status.uptime_secs / 60) % 60}m ${status.uptime_secs % 60}s`,
      ],
      ["Contacts / groups", `${status.contacts} / ${status.groups}`],
      ["Database / version", `${status.database_driver} / v${status.version}`],
    ]) {
      const row = element("div", "");
      row.append(element("dt", label), element("dd", value));
      byId("runtime").append(row);
    }
    maxMediaBytes = status.media_max_bytes;
    byId("media-limit").textContent =
      `Server file limit: ${Math.floor(maxMediaBytes / 1024 / 1024)} MiB.`;
    byId("last-update").textContent =
      `Updated ${new Date().toLocaleTimeString("en-US")}`;
    renderRows(
      "jobs-body",
      jobs.map((job) => {
        let cancel = "";
        if (job.status === "pending") {
          cancel = document.createElement("button");
          cancel.type = "button";
          cancel.className = "secondary";
          cancel.textContent = "Cancel";
          cancel.setAttribute("aria-label", `Cancel message ${job.id}`);
          cancel.onclick = () =>
            action(cancel, async () => {
              await api(`messages/${encodeURIComponent(job.id)}`, "DELETE");
              await refresh();
              notice("Message cancelled.");
            });
        }
        return [
          job.id,
          new Date(job.send_at * 1000).toLocaleString("en-US"),
          badge(job.status, job.status),
          job.failure_code || "—",
          cancel,
        ];
      }),
      "No messages in the queue yet.",
      5,
    );
    if (!byId("pause-logs").checked) {
      logsSnapshot = logs;
      renderLogs();
    }
  } finally {
    polling = false;
  }
}

async function loadLanguages(selected = byId("language").value) {
  const response = await api("languages");
  if (!token) return;
  byId("language").replaceChildren(...response.languages.map((language) => {
    const option = document.createElement("option");
    option.value = language.code;
    option.textContent = language.ready
      ? language.name
      : `${language.name} (${language.code}, generates on save)`;
    return option;
  }));
  byId("language").value = selected;
  byId("language-hint").textContent = response.warning ||
    "New languages are generated automatically when you save. Cached translations work without the translation service.";
}

byId("reload-languages").onclick = (event) => action(event.currentTarget, async () => {
  await loadLanguages();
  notice("Language list refreshed.");
});

byId("login-form").addEventListener("submit", (event) => {
  event.preventDefault();
  action(event.submitter, async () => {
    token = byId("token").value.trim();
    let settings, commands, sticker;
    try {
      [settings, commands, sticker] = await Promise.all([
        api("settings"),
        api("commands"),
        api("sticker"),
      ]);
      await loadLanguages(settings.language);
    } catch (error) {
      token = "";
      throw error;
    }
    if (!token) return;
    byId("token").value = "";
    byId("bot-name").value = settings.name;
    byId("owners").value = settings.owners.join(", ");
    byId("prefix").value = settings.prefix;
    byId("mode").value = settings.mode;
    byId("daily-limit").value = settings.daily_limit;
    byId("sticker-packname").value = sticker.packname;
    byId("sticker-authorname").value = sticker.authorname;
    commandsSnapshot = commands;
    renderCommands();
    navigate();
    byId("login-panel").hidden = true;
    byId("workspace").hidden = false;
    document.querySelector(".skip-link").href = "#page-title";
    await refresh();
    notice("Dashboard connected.");
    byId("refresh").focus();
  });
});

byId("logout").onclick = logout;
byId("refresh").onclick = (event) =>
  action(event.currentTarget, async () => {
    await refresh();
    notice("Status refreshed.");
  });
byId("reconnect").onclick = (event) =>
  action(event.currentTarget, async () => {
    await api("reconnect", "POST");
    await refresh();
    notice("Reconnect requested.");
  });

byId("settings-form").addEventListener("submit", (event) => {
  event.preventDefault();
  action(event.submitter, async () => {
    await api("settings", "PUT", {
      name: byId("bot-name").value,
      owners: byId("owners")
        .value.split(",")
        .map((s) => s.trim())
        .filter(Boolean),
      prefix: byId("prefix").value,
      language: byId("language").value,
      mode: byId("mode").value,
      daily_limit: Number(byId("daily-limit").value),
    });
    await refresh();
    await loadLanguages(byId("language").value);
    notice("Settings saved and applied.");
  });
});

byId("sticker-form").addEventListener("submit", (event) => {
  event.preventDefault();
  action(event.submitter, async () => {
    await api("sticker", "PUT", {
      packname: byId("sticker-packname").value,
      authorname: byId("sticker-authorname").value,
    });
    notice("Sticker settings saved and applied.");
  });
});

function updateKind() {
  const media = byId("kind").value !== "text";
  byId("media-field").hidden = !media;
  byId("media-file").required = media;
  byId("mimetype").required = media;
  byId("message").required = !media;
}
byId("kind").onchange = updateKind;
byId("media-file").onchange = () => {
  const file = byId("media-file").files[0];
  byId("mimetype").value = file?.type || "application/octet-stream";
};

byId("send-form").addEventListener("submit", (event) => {
  event.preventDefault();
  action(event.submitter, async () => {
    const kind = byId("kind").value;
    const file = byId("media-file").files[0];
    const payload = {
      to: byId("recipient").value,
      kind,
      text: byId("message").value,
      send_at: byId("send-at").value
        ? new Date(byId("send-at").value).toISOString()
        : null,
    };
    if (kind !== "text") {
      if (!file) throw new Error("Choose a media file.");
      if (file.size > maxMediaBytes)
        throw new Error(
          `File too large. The server limit is ${Math.floor(maxMediaBytes / 1024 / 1024)} MiB.`,
        );
      payload.data_base64 = await new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result).split(",")[1]);
        reader.onerror = () => reject(new Error("The file could not be read."));
        reader.readAsDataURL(file);
      });
      payload.file_name = file.name;
      payload.mimetype = byId("mimetype").value;
    }
    const result = await api("messages", "POST", payload);
    byId("send-form").reset();
    updateKind();
    await refresh();
    notice(`Message queued: ${result.id}`);
  });
});

byId("timezone").textContent =
  `Local time: ${Intl.DateTimeFormat().resolvedOptions().timeZone}. Leave blank to send when WhatsApp is connected.`;
setInterval(() => {
  if (token) refresh().catch((error) => notice(error.message, true));
}, 5000);
