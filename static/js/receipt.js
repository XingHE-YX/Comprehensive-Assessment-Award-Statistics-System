document.querySelectorAll("[data-copy-target]").forEach((button) => {
  button.addEventListener("click", async () => {
    const target = document.getElementById(button.dataset.copyTarget);
    if (!target) return;
    try { await navigator.clipboard.writeText(target.textContent || ""); button.textContent = "已复制"; }
    catch (_) { button.textContent = "请手动复制"; }
  });
});
