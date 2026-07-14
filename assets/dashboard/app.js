"use strict";

const byId = (id) => document.getElementById(id);

const label = (value) => String(value || "unknown").replaceAll("_", " ");

const titleLabel = (value) => {
  const text = label(value);
  return text.charAt(0).toUpperCase() + text.slice(1);
};

const clearChildren = (node) => {
  while (node.firstChild) {
    node.removeChild(node.firstChild);
  }
};

const createText = (tag, className, value) => {
  const node = document.createElement(tag);
  if (className) {
    node.className = className;
  }
  node.textContent = value;
  return node;
};

const summarizeFindings = (findings) => {
  if (!Array.isArray(findings) || findings.length === 0) {
    return { title: "Structure accepted", detail: "No protected class was present", action: "allow" };
  }

  const first = findings[0];
  const total = findings.reduce((sum, finding) => sum + Number(finding.count || 0), 0);
  const extra = findings.length > 1 ? ` · ${findings.length} classes` : "";
  return {
    title: `${titleLabel(first.data_class)} protected`,
    detail: `${total} finding${total === 1 ? "" : "s"} · ${label(first.source_field)}${extra}`,
    action: label(first.action),
  };
};

const renderActivity = (activity) => {
  const list = byId("activity-list");
  if (!list || !Array.isArray(activity) || activity.length === 0) {
    return;
  }

  clearChildren(list);
  activity.slice(0, 8).forEach((event) => {
    const summary = summarizeFindings(event.findings);
    const row = document.createElement("article");
    row.className = "activity-row";

    row.appendChild(createText("span", "activity-sequence", `#${String(event.request_sequence).padStart(3, "0")}`));

    const copy = document.createElement("div");
    copy.className = "activity-copy";
    copy.appendChild(createText("strong", "", summary.title));
    copy.appendChild(createText("small", "", `${summary.detail} · ${label(event.upstream_outcome)} · ${event.scan_latency_ms}ms`));
    row.appendChild(copy);

    const action = createText("span", `activity-action ${summary.action}`, summary.action);
    row.appendChild(action);
    list.appendChild(row);
  });
};

const applyState = (state) => {
  byId("session-id").textContent = state.session_pseudonym;
  byId("original-count").textContent = String(state.originals_forwarded);
  byId("audit-state").textContent = state.audit_healthy ? "Healthy" : "Degraded";
  byId("transport-state").textContent = state.transport === "responses_sse" ? "Responses" : titleLabel(state.transport);
  byId("request-count").textContent = String(state.request_count);
  byId("policy-name").textContent = state.policy_name;
  byId("last-updated").textContent = `· ${titleLabel(state.upstream)} upstream`;
  renderActivity(state.activity);
};

const loadState = async () => {
  try {
    const response = await fetch("/dashboard/state", {
      cache: "no-store",
      credentials: "omit",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error("state unavailable");
    }
    applyState(await response.json());
  } catch (_error) {
    byId("audit-state").textContent = "Unavailable";
    byId("last-updated").textContent = "· State unavailable";
  }
};

loadState();
window.setInterval(loadState, 2000);
