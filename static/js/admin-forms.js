"use strict";
for (const form of document.querySelectorAll("[data-admin-form]")) {
  form.addEventListener("submit", (event) => {
    const button = event.submitter;
    if (!button) return;
    button.dataset.originalLabel = button.textContent;
    button.disabled = true;
    button.textContent = "提交中…";
  });
}
window.addEventListener("pageshow", () => {
  for (const button of document.querySelectorAll("button[data-original-label]")) {
    button.textContent = button.dataset.originalLabel;
    button.disabled = false;
  }
});
