const form = document.querySelector("#submission-form");

if (form) {
  const category = form.querySelector("#category");
  const resultChoice = form.querySelectorAll('input[name="has_result"]');
  const resultFields = form.querySelector("[data-result-fields]");
  const declaration = form.querySelector("#declaration-confirm");
  const noResultConfirm = form.querySelector("#no-result-confirm");
  const sections = [...form.querySelectorAll("[data-category-section]")];

  const setDisabled = (root, disabled) => {
    root.querySelectorAll("input, select, textarea, button").forEach((control) => {
      control.disabled = disabled;
    });
  };

  const syncCategory = () => {
    const selected = category?.value || "";
    sections.forEach((section) => {
      const active = section.dataset.categorySection === selected;
      section.hidden = !active;
      setDisabled(section, !active);
    });

    const articleNature = form.querySelector("#article-nature")?.value || "";
    form.querySelectorAll("[data-article-fields]").forEach((section) => {
      const active = section.dataset.articleFields === articleNature;
      section.hidden = !active;
      setDisabled(section, !active);
    });

    const certificateType = form.querySelector("#certificate-type")?.value || "";
    form.querySelectorAll("[data-certificate-fields]").forEach((section) => {
      const active = section.dataset.certificateFields === certificateType;
      section.hidden = !active;
      setDisabled(section, !active);
    });
  };

  const syncResultChoice = () => {
    const hasResult = form.querySelector('input[name="has_result"]:checked')?.value === "yes";
    if (resultFields) {
      resultFields.hidden = !hasResult;
      setDisabled(resultFields, !hasResult);
    }
    if (declaration) declaration.hidden = hasResult;
    if (noResultConfirm) noResultConfirm.disabled = hasResult;
    if (category) category.disabled = !hasResult;
    if (hasResult) syncCategory();
  };

  category?.addEventListener("change", syncCategory);
  form.querySelector("#article-nature")?.addEventListener("change", syncCategory);
  form.querySelector("#certificate-type")?.addEventListener("change", syncCategory);
  resultChoice.forEach((choice) => choice.addEventListener("change", syncResultChoice));
  form.addEventListener("submit", (event) => {
    const submitter = event.submitter;
    if (submitter instanceof HTMLButtonElement) {
      submitter.disabled = true;
      submitter.textContent = "提交中…";
    }
  });
  syncResultChoice();
}
