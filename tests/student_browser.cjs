const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require(process.env.ZONGCE_PLAYWRIGHT_MODULE || "playwright");

const baseUrl = process.argv[2];
if (!baseUrl) throw new Error("Pass the URL from cargo run --example student_preview");
const output = process.env.ZONGCE_SCREENSHOTS || "/tmp/zongce-student-screenshots";
fs.mkdirSync(output, { recursive: true });
const cases = [
  ["academic_competition", { competition_name: "测试竞赛", competition_type: "A", level: "国家", award_level: "其他", other_award: "第四名" }],
  ["sports_arts_competition", { competition_name: "测试比赛", level: "校", has_award_level: "no", rank: "第八名" }],
  ["other_award", { award_name: "测试荣誉", recognition_level: "校级", school_honor_category: "其他", is_scholarship: "yes" }],
  ["published_article", { title: "测试论文", nature: "academic", publication_type: "期刊", author_order: "第一作者", journal_name: "测试期刊" }],
  ["social_practice", { project_name: "测试实践", level: "校级", identity: "队员", award_level_or_none: "无具体等级" }],
  ["patent", { name: "测试专利", type: "发明", status: "申请中", ranking: "1", patent_no: "TEST-PATENT-001" }],
  ["certification", { certificate_type: "computer", computer_category: "非计算机", exam_level: "二级" }],
  ["published_article", { title: "测试文章", nature: "non_academic", platform: "测试刊物", publication_form: "纸刊", link_or_info: "2026年第一期" }],
  ["certification", { certificate_type: "CET-6", cet6_score: "500" }],
  ["certification", { certificate_type: "雅思/托福", language_score: "7" }],
  ["certification", { certificate_type: "other", qualification_name: "测试资格证书" }],
];

(async () => {
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  try {
    for (const [width, height] of [[320, 568], [390, 844], [768, 1024], [1440, 900]]) {
      const context = await browser.newContext({ viewport: { width, height } });
      const page = await context.newPage();
      page.setDefaultTimeout(8000);
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      const checkWidth = async () => {
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "Horizontal overflow at " + width);
      };
      await page.goto(baseUrl);
      await checkWidth();
      await page.screenshot({ path: path.join(output, width + "-home.png"), fullPage: true });
      await page.locator("#access-code").fill("preview-only");
      await page.getByRole("button", { name: "继续填写" }).click();
      await page.waitForURL("**/submit");
      const samples = width === 1440 ? cases : cases.slice(0, 2);
      for (const [category, fields] of samples) {
        await page.goto(baseUrl + "/submit");
        await page.locator("#student_name").fill("测试学生");
        await page.locator("#student_no").fill("TEST-" + width);
        await page.locator("#result_name").fill("测试成果 <script>不可执行</script>");
        await page.locator("#obtained_date").fill("2026-04-02");
        await page.locator("#category").selectOption(category);
        for (const [key, value] of Object.entries(fields)) {
          const control = page.locator("#" + category + "-" + key);
          if (await control.evaluate((element) => element.tagName === "SELECT")) await control.selectOption(value);
          else await control.fill(value);
        }
        await page.locator("#attachments").setInputFiles({
          name: "测试证明.pdf",
          mimeType: "application/pdf",
          buffer: Buffer.from("%PDF-1.4\n% browser test fixture\n%%EOF"),
        });
        await checkWidth();
        await page.getByRole("button", { name: "提交申报", exact: true }).click();
        await page.waitForURL("**/success/ZC*");
        const number = await page.locator(".submission-number").innerText();
        const code = await page.locator("#edit-code").innerText();
        await page.goto(baseUrl + "/query");
        await page.locator("#submission-no").fill(number);
        await page.locator("#edit-code").fill(code);
        await page.getByRole("button", { name: "查询申报" }).click();
        await page.waitForURL("**/query/ZC*");
        assert.equal(await page.locator(".status-badge").innerText(), "待审核");
        const attachmentUrl = await page.locator(".attachment-list a").first().getAttribute("href");
        const attachment = await context.request.get(baseUrl + attachmentUrl);
        assert.equal(attachment.status(), 200);
        assert.equal(attachment.headers()["cache-control"], "no-store");
        await checkWidth();
        if (category === "academic_competition") await page.screenshot({ path: path.join(output, width + "-detail.png"), fullPage: true });
        await page.getByRole("link", { name: "编辑申报", exact: true }).click();
        assert(await page.locator("#edit-submission").evaluate((element) => element.open));
        await page.locator("#edit-submission > summary").click();
        await page.getByRole("link", { name: "编辑申报", exact: true }).click();
        assert(await page.locator("#edit-submission").evaluate((element) => element.open), "Repeated Edit action must reopen the editor");
        for (const [key, value] of Object.entries(fields)) {
          assert.equal(await page.locator("#" + category + "-" + key).inputValue(), value);
        }
        await page.locator("#result_name").fill("浏览器修改已保存");
        await checkWidth();
        if (category === "academic_competition") await page.screenshot({ path: path.join(output, width + "-edit.png"), fullPage: true });
        await page.getByRole("button", { name: "保存并重新提交" }).click();
        await page.waitForURL((url) => url.pathname === "/query/" + number && !url.hash);
        assert(await page.getByText("已重新提交审核。", { exact: true }).isVisible());
        assert.equal(await page.locator(".attachment-list li").count(), 1);
        assert(await page.getByText("浏览器修改已保存", { exact: true }).isVisible());
      }
      await page.goto(baseUrl + "/submit");
      await page.locator("#student_name").fill("无材料测试学生");
      await page.locator("#student_no").fill("TEST-NONE-" + width);
      await page.locator('input[name="has_result"][value="no"]').check();
      await page.locator("#no-result-confirm").check();
      await page.getByRole("button", { name: "提交申报", exact: true }).click();
      await page.waitForURL("**/success/declaration");
      assert.deepEqual(errors, []);
      await context.close();
      console.log(width + "x" + height + ": submission, query, edit, protected download and declaration passed");
    }
  } finally {
    await browser.close();
  }
})().catch((error) => { console.error(error.message); process.exitCode = 1; });
