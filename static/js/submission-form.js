const form = document.querySelector("#submission-form");

if (form) {
  const category = form.querySelector("#category");
  const resultFields = form.querySelector("[data-result-fields]");
  const declaration = form.querySelector("#declaration-confirm");
  const noResultConfirm = form.querySelector("#no-result-confirm");
  const sections = [...form.querySelectorAll("[data-category-section]")];
  for (const fallback of form.querySelectorAll("[data-form-refresh]")) {
    fallback.hidden = true;
    fallback.querySelector("button").disabled = true;
    fallback.querySelector("button").type = "button";
  }
  const sync = () => {
    const hasResult = form.dataset.editing === "true" || form.querySelector('input[name="has_result"]:checked')?.value !== "no";
    resultFields.hidden = !hasResult;
    resultFields.disabled = !hasResult;
    resultFields.querySelectorAll("input, select, textarea").forEach((control) => { control.disabled = !hasResult; });
    if (declaration) declaration.hidden = hasResult;
    if (noResultConfirm) noResultConfirm.disabled = hasResult;
    sections.forEach((section) => {
      const active = hasResult && section.dataset.categorySection === category.value;
      section.hidden = !active;
      section.disabled = !active;
      section.querySelectorAll("[data-condition-key]").forEach((field) => {
        const key = field.dataset.conditionKey;
        const discriminator = [...section.querySelectorAll("[name]")].find((control) => control.name === key);
        const value = discriminator?.value.trim();
        const matches = value === field.dataset.conditionValue
          || (key === "recognition_level" && value === "校" && field.dataset.conditionValue === "校级")
          || (key === "certificate_type" && value === "CET-6" && field.dataset.conditionValue === "CET-4/CET-6");
        const visible = active && (!key || matches || field.dataset.preserveValue === "true");
        field.hidden = !visible;
        field.querySelectorAll("input, select, textarea").forEach((control) => {
          control.disabled = !visible;
          control.required = visible && control.dataset.required === "true";
        });
      });
    });
  };
  form.addEventListener("change", sync);
  form.addEventListener("input", (event) => {
    if (event.target.name === "recognition_level") sync();
  });
  form.addEventListener("submit", (event) => {
    if (event.submitter instanceof HTMLButtonElement) {
      event.submitter.disabled = true;
      event.submitter.textContent = "提交中…";
    }
  });
  sync();
}

const openEditor = () => {
  const editor = document.querySelector("#edit-submission");
  if (location.hash === "#edit-submission" && editor) editor.open = true;
};
window.addEventListener("hashchange", openEditor);
document.querySelector('a[href="#edit-submission"]')?.addEventListener("click", () => {
  const editor = document.querySelector("#edit-submission");
  if (editor) editor.open = true;
});
openEditor();
