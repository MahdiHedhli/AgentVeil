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

const applyState = (state) => {
  if (!state || state.protected !== true || typeof state.session_pseudonym !== "string") {
    throw new Error("invalid state");
  }
  const syntheticProof = state.wire_proof === "synthetic_passed";
  const syntheticRoute = state.upstream === "loopback_test";
  byId("session-state-label").textContent = "Protected session";
  byId("route-state").lastChild.textContent = "Active";
  byId("session-id").textContent = state.session_pseudonym;
  byId("wire-proof-state").textContent = syntheticProof ? "Passed" : "Not measured";
  byId("wire-proof-detail").textContent = syntheticProof
    ? "Capturing upstream excluded protected originals"
    : "Run the closed-loop demo for evidence";
  byId("audit-state").textContent = state.audit_healthy ? "Healthy" : "Degraded";
  byId("transport-state").textContent = state.transport === "responses_sse" ? "Responses" : titleLabel(state.transport);
  byId("request-count").textContent = String(state.request_count);
  byId("policy-name").textContent = state.policy_name;
  byId("route-title").textContent = syntheticRoute ? "Synthetic client to loopback proof" : "Local client to OpenAI Responses";
  byId("route-origin").textContent = syntheticRoute ? "Synthetic client" : "Local Responses client";
  byId("route-origin-detail").textContent = syntheticRoute ? "Offline fixture harness" : "Configured caller";
  byId("route-destination").textContent = syntheticRoute ? "Loopback fixture" : "OpenAI Responses";
  byId("route-destination-detail").textContent = syntheticRoute ? "Capturing fake upstream" : "Fixed upstream";
  byId("route-flow").setAttribute(
    "aria-label",
    syntheticRoute
      ? "Synthetic client routes through AgentVeil to a capturing loopback fixture"
      : "A local Responses client routes through AgentVeil to the fixed OpenAI Responses upstream",
  );
  document.querySelectorAll('[data-proof-scope="synthetic"] .proof-status').forEach((node) => {
    node.textContent = syntheticProof ? "Proven this run" : "Configured";
  });
  document.querySelectorAll('[data-proof-scope="synthetic"] .check').forEach((node) => {
    node.textContent = syntheticProof ? "✓" : "•";
    node.classList.toggle("verified", syntheticProof);
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
  byId("route-title").textContent = "State unavailable";
  document.querySelectorAll('[data-proof-scope="synthetic"] .proof-status').forEach((node) => {
    node.textContent = "Unavailable";
  });
  document.querySelectorAll('[data-proof-scope="synthetic"] .check').forEach((node) => {
    node.textContent = "•";
    node.classList.remove("verified");
  });
  byId("last-updated").textContent = "· State unavailable";
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

loadState();
window.setInterval(loadState, 2000);
