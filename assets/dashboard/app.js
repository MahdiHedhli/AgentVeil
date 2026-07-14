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

const summarizeFindings = (findings, decision) => {
  if (decision === "rejected") {
    return { title: "Request rejected", detail: "Unsupported or unsafe shape stopped locally", action: "rejected" };
  }
  if (decision === "blocked" && (!Array.isArray(findings) || findings.length === 0)) {
    return { title: "Request blocked", detail: "Protected egress did not start", action: "block" };
  }
  if (!Array.isArray(findings) || findings.length === 0) {
    return { title: "Structure accepted", detail: "No protected class was present", action: "allow" };
  }

  const ownedTokenFindings = findings.filter(
    (finding) => finding.data_class === "token_namespace" && finding.action === "allow",
  );
  if (ownedTokenFindings.length > 0) {
    const ownedTotal = ownedTokenFindings.reduce((sum, finding) => sum + Number(finding.count || 0), 0);
    const reprotectedTotal = findings
      .filter((finding) => finding.data_class !== "token_namespace")
      .reduce((sum, finding) => sum + Number(finding.count || 0), 0);
    return {
      title: "Protected history replayed",
      detail: reprotectedTotal > 0
        ? `${ownedTotal} owned token${ownedTotal === 1 ? "" : "s"} · ${reprotectedTotal} prior value${reprotectedTotal === 1 ? "" : "s"} re-protected`
        : `${ownedTotal} owned token${ownedTotal === 1 ? "" : "s"} · same session · TTL checked`,
      action: decision === "rewritten" ? "tokenize" : "allow",
    };
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
    const summary = summarizeFindings(event.findings, event.decision);
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

const renderActivityUnavailable = () => {
  const list = byId("activity-list");
  if (!list) {
    return;
  }
  clearChildren(list);
  const empty = createText("div", "empty-state", "");
  const mark = createText("span", "empty-mark", "");
  mark.setAttribute("aria-hidden", "true");
  empty.appendChild(mark);
  empty.appendChild(createText("p", "", "Activity unavailable."));
  empty.appendChild(createText("small", "", "Dashboard state could not be verified."));
  list.appendChild(empty);
};

const updateNavigationState = () => {
  const section = window.location.hash || "#overview";
  document.querySelectorAll(".nav-item").forEach((node) => {
    const active = node.getAttribute("href") === section;
    node.classList.toggle("active", active);
    if (active) {
      node.setAttribute("aria-current", "page");
    } else {
      node.removeAttribute("aria-current");
    }
  });
};

const applyState = (state) => {
  if (
    !state ||
    state.schema_version !== 2 ||
    state.protected !== true ||
    state.transport !== "responses_sse" ||
    !["openai", "loopback_test"].includes(state.upstream) ||
    !["generic", "codex_cli"].includes(state.client) ||
    !["disabled", "synthetic_full", "synthetic_display_delta_only"].includes(state.restoration) ||
    !["not_measured", "synthetic_passed", "synthetic_failed"].includes(state.wire_proof) ||
    typeof state.audit_healthy !== "boolean" ||
    typeof state.session_pseudonym !== "string" ||
    typeof state.policy_name !== "string" ||
    !Number.isInteger(state.request_count) ||
    state.request_count < 0 ||
    !Array.isArray(state.activity)
  ) {
    throw new Error("invalid state");
  }
  const syntheticProof = state.wire_proof === "synthetic_passed";
  const syntheticProofFailed = state.wire_proof === "synthetic_failed";
  const syntheticRoute = state.upstream === "loopback_test";
  const syntheticCodexRoute =
    state.client === "codex_cli" &&
    syntheticRoute &&
    state.restoration === "synthetic_display_delta_only";
  byId("session-state-label").textContent = "Protected session";
  byId("route-state").lastChild.textContent = "Active";
  byId("session-id").textContent = state.session_pseudonym;
  byId("wire-proof-state").textContent = syntheticProofFailed
    ? "Failed"
    : syntheticProof ? "Passed" : "Not measured";
  byId("wire-proof-detail").textContent = syntheticProofFailed
    ? "Capturing boundary observed forbidden content · discard this run"
    : syntheticProof
      ? "Capturing upstream excluded protected originals"
      : "Run the closed-loop demo for evidence";
  byId("audit-state").textContent = state.audit_healthy ? "Healthy" : "Degraded";
  byId("transport-label").textContent = syntheticCodexRoute ? "Restoration" : "Transport";
  byId("transport-state").textContent = syntheticCodexRoute
    ? "Display only"
    : state.transport === "responses_sse" ? "Responses" : titleLabel(state.transport);
  byId("transport-detail").textContent = syntheticCodexRoute
    ? "Exact token · same session · TTL-bound"
    : "SSE streaming preserved";
  byId("request-count").textContent = String(state.request_count);
  byId("policy-name").textContent = state.policy_name;
  byId("route-title").textContent = syntheticCodexRoute
    ? "Codex CLI to synthetic loopback proof"
    : syntheticRoute ? "Synthetic client to loopback proof" : "Local client to OpenAI Responses";
  byId("route-origin").textContent = syntheticCodexRoute
    ? "Codex CLI"
    : syntheticRoute ? "Synthetic client" : "Local Responses client";
  byId("route-origin-detail").textContent = syntheticCodexRoute
    ? "Private isolated session"
    : syntheticRoute ? "Offline fixture harness" : "Configured caller";
  byId("route-destination").textContent = syntheticRoute ? "Loopback fixture" : "OpenAI Responses";
  byId("route-destination-detail").textContent = syntheticRoute ? "Capturing loopback fixture" : "Fixed upstream";
  byId("route-flow").setAttribute(
    "aria-label",
    syntheticCodexRoute
      ? "Codex CLI routes through AgentVeil to a capturing synthetic loopback fixture"
      : syntheticRoute
      ? "Synthetic client routes through AgentVeil to a capturing loopback fixture"
      : "A local Responses client routes through AgentVeil to the fixed OpenAI Responses upstream",
  );
  byId("restoration-state").textContent = syntheticCodexRoute
    ? "Synthetic display restoration · model route loopback-only"
    : state.restoration === "synthetic_full"
      ? "Synthetic harness restoration"
      : state.restoration === "synthetic_display_delta_only"
        ? "Synthetic delta restoration"
        : "Live restoration is disabled";
  const fullSyntheticProof = syntheticProof && !syntheticCodexRoute;
  document.querySelectorAll('[data-proof-scope="synthetic"] .proof-status').forEach((node) => {
    node.textContent = syntheticProofFailed
      ? "Failed"
      : fullSyntheticProof ? "Proven this run" : "Configured";
  });
  document.querySelectorAll('[data-proof-scope="synthetic"] .check').forEach((node) => {
    node.textContent = syntheticProofFailed ? "×" : fullSyntheticProof ? "✓" : "•";
    node.classList.toggle("verified", fullSyntheticProof);
  });
  byId("last-updated").textContent = `· ${titleLabel(state.upstream)} upstream`;
  renderActivity(state.activity);
};

const markStateUnavailable = () => {
  byId("session-state-label").textContent = "Session state unavailable";
  byId("route-state").lastChild.textContent = "Unavailable";
  byId("session-id").textContent = "unavailable";
  byId("wire-proof-state").textContent = "Unavailable";
  byId("wire-proof-detail").textContent = "Dashboard state could not be verified";
  byId("audit-state").textContent = "Unavailable";
  byId("transport-state").textContent = "Unavailable";
  byId("transport-label").textContent = "Transport";
  byId("transport-detail").textContent = "Dashboard state could not be verified";
  byId("restoration-state").textContent = "Restoration state unavailable";
  byId("route-title").textContent = "State unavailable";
  byId("route-origin").textContent = "Unavailable";
  byId("route-origin-detail").textContent = "Dashboard state could not be verified";
  byId("route-destination").textContent = "Unavailable";
  byId("route-destination-detail").textContent = "Dashboard state could not be verified";
  byId("route-flow").setAttribute("aria-label", "Dashboard route state unavailable");
  byId("request-count").textContent = "—";
  byId("policy-name").textContent = "Unavailable";
  document.querySelectorAll('[data-proof-scope="synthetic"] .proof-status').forEach((node) => {
    node.textContent = "Unavailable";
  });
  document.querySelectorAll('[data-proof-scope="synthetic"] .check').forEach((node) => {
    node.textContent = "•";
    node.classList.remove("verified");
  });
  byId("last-updated").textContent = "· State unavailable";
  renderActivityUnavailable();
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
    markStateUnavailable();
  }
};

updateNavigationState();
window.addEventListener("hashchange", updateNavigationState);
loadState();
window.setInterval(loadState, 2000);
