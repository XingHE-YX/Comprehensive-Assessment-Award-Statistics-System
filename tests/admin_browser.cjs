const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const { randomBytes } = require("node:crypto");
const { chromium } = require(process.env.ZONGCE_PLAYWRIGHT_MODULE || "playwright");
const password = randomBytes(24).toString("hex");
const output = process.env.ZONGCE_SCREENSHOTS || "/tmp/zongce-admin-screenshots";
fs.mkdirSync(output, { recursive: true });
const preview = spawn(process.env.ZONGCE_PREVIEW_BIN || "target/debug/examples/admin_preview", [], {
  env: { ...process.env, ZONGCE_PREVIEW_PASSWORD: password }, stdio: ["ignore", "pipe", "pipe"],
});
const baseUrl = new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(new Error("Preview did not start")), 15000);
  let stdout = "";
  preview.stdout.on("data", (chunk) => {
    stdout += chunk;
    const match = stdout.match(/Admin preview: (http:\/\/127\.0\.0\.1:\d+)/);
    if (match) { clearTimeout(timeout); resolve(match[1]); }
  });
  preview.on("error", (error) => { clearTimeout(timeout); reject(error); });
  preview.on("exit", (code) => { clearTimeout(timeout); reject(new Error("Preview exited: " + code)); });
});

(async () => {
  let browser;
  try {
    const base = await baseUrl;
    browser = await chromium.launch({ channel: "chrome", headless: true });
    for (const [width, height] of [[320,568], [390,844], [768,1024], [1440,900]]) {
      const studentContext = await browser.newContext({ viewport: { width, height } });
      const adminContext = await browser.newContext({ viewport: { width, height }, javaScriptEnabled: width !== 768 });
      const student = await studentContext.newPage();
      const admin = await adminContext.newPage();
      student.setDefaultTimeout(8000); admin.setDefaultTimeout(8000);
      const errors = [];
      admin.on("pageerror", error => errors.push(error.message));
      const checkWidth = async () => assert(await admin.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "Page overflow at " + width);
      await student.goto(base);
      await student.locator("#access-code").fill("preview-only");
      await student.getByRole("button", { name: "继续填写" }).click();
      await student.waitForURL("**/submit");
      await student.locator("#student_name").fill("后台流程测试");
      await student.locator("#student_no").fill("ADMIN-TEST-" + width);
      await student.locator("#result_name").fill("测试审核成果 <script>不可执行</script>");
      await student.locator("#obtained_date").fill("2026-04-02");
      await student.locator("#category").selectOption("academic_competition");
      await student.locator("#academic_competition-competition_name").fill("测试竞赛");
      for (const [key, value] of [["competition_type","A"],["level","国家"],["award_level","一等奖"]]) await student.locator("#academic_competition-" + key).selectOption(value);
      await student.locator("#attachments").setInputFiles([
        { name: "测试图片.png", mimeType: "image/png", buffer: Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=", "base64") },
        { name: "测试材料.pdf", mimeType: "application/pdf", buffer: Buffer.from("%PDF-1.4\n% fixture\n%%EOF") },
      ]);
      await student.getByRole("button", {name:"提交申报",exact:true}).click();
      await student.waitForURL("**/success/ZC*");
      const number = await student.locator(".submission-number").innerText();
      const code = await student.locator("#edit-code").innerText();
      await student.goto(base + "/query");
      await student.locator("#submission-no").fill(number);
      await student.locator("#edit-code").fill(code);
      await student.getByRole("button", {name:"查询申报"}).click();
      await student.waitForURL("**/query/ZC*");
      const studentUrl = student.url();
      const denied = await studentContext.request.post(base + "/admin/submissions/1/review", { form: { status:"approved", approved_score:"99" } });
      assert.equal(denied.status(),403);

      await admin.goto(base + "/admin");
      assert(admin.url().endsWith("/admin/login"));
      await admin.locator("#username").fill("preview-admin");
      await admin.locator("#password").fill("invalid-preview-password");
      const failedLogin = admin.waitForResponse(response => response.url().endsWith("/admin/login") && response.request().method() === "POST");
      await admin.getByRole("button", {name:"登录",exact:true}).click();
      assert.equal((await failedLogin).status(),401);
      await admin.locator("#login-error").waitFor();
      assert.equal(await admin.locator("#password").inputValue(), "");
      await checkWidth();
      await admin.screenshot({path:path.join(output,width+"-login.png"),fullPage:true});
      await admin.locator("#password").fill(password);
      await admin.getByRole("button", {name:"登录",exact:true}).click();
      await admin.waitForURL(base + "/admin");
      await admin.locator("#student_no").fill("ADMIN-TEST-" + width);
      await admin.getByRole("button",{name:"应用筛选"}).click();
      await admin.waitForURL("**/admin?*");
      assert.equal(await admin.locator("#count-total").innerText(),"1");
      assert.equal(await admin.locator("#count-pending").innerText(),"1");
      assert.equal(await admin.locator("#approved-total").innerText(),"0.00");
      await checkWidth();
      await admin.screenshot({path:path.join(output,width+"-dashboard.png"),fullPage:true});
      if (width === 320) assert(await admin.locator(".table-scroll").evaluate(el => el.scrollWidth > el.clientWidth));
      await admin.getByRole("link",{name:"查看 " + number,exact:true}).click();
      await admin.waitForURL("**/admin/submissions/*");
      const detailUrl = admin.url();
      await admin.locator(".attachment-preview").scrollIntoViewIfNeeded();
      await admin.waitForFunction(() => document.querySelector(".attachment-preview").naturalWidth > 0);
      const attachmentUrl = await admin.locator(".admin-attachments a").last().getAttribute("href");
      assert.equal((await adminContext.request.get(base + attachmentUrl)).status(),200);
      const publicContext = await browser.newContext();
      assert.equal((await publicContext.request.get(base + attachmentUrl)).status(),403);
      await publicContext.close();
      await checkWidth();
      await admin.screenshot({path:path.join(output,width+"-detail.png"),fullPage:true});
      const review = async (status, score, note, expected) => {
        await admin.locator("#review-status").selectOption(status);
        await admin.locator("#review-approved_score").fill(score);
        await admin.locator("#review-review_note").fill(note);
        const saved = admin.waitForResponse(response => response.url().endsWith("/review") && response.request().method() === "POST");
        await admin.getByRole("button",{name:"保存审核"}).click();
        assert.equal((await saved).status(),expected);
        if (expected === 303) await admin.waitForURL(url => url.pathname === new URL(detailUrl).pathname && url.searchParams.get("saved") === "1");
        else await admin.locator("#score-error").waitFor();
      };
      await review("approved", "", "测试审核备注",422);
      assert.equal(await admin.locator("#review-review_note").inputValue(),"测试审核备注");
      assert.equal(await admin.locator("#review-approved_score").getAttribute("aria-invalid"),"true");
      await checkWidth();
      await admin.screenshot({path:path.join(output,width+"-validation.png"),fullPage:true});
      await review("needs_revision","","请补充说明",303);
      await student.goto(studentUrl);
      assert.equal(await student.locator(".status-badge").innerText(),"需补充材料");
      await student.getByRole("link",{name:"编辑申报",exact:true}).click();
      await student.locator("#result_name").fill("补充后的测试成果");
      await student.getByRole("button",{name:"保存并重新提交",exact:true}).click();
      await student.waitForURL(studentUrl);
      await admin.goto(detailUrl);
      assert.equal(await admin.locator(".detail-status .status-badge").innerText(),"待审核");
      assert((await admin.locator("main").innerText()).includes("学生已重新提交审核"));
      assert.equal(await admin.locator("#review-review_note").inputValue(),"请补充说明");
      await review("approved","2.35","核定通过",303);
      await student.goto(studentUrl);
      assert.equal(await student.locator(".status-badge").innerText(),"已通过");
      assert.equal(await student.getByRole("link",{name:"编辑申报",exact:true}).count(),0);
      await admin.goto(base + "/admin?student_no=ADMIN-TEST-" + width + "&status=approved");
      assert.equal(await admin.locator("#count-approved").innerText(),"1");
      assert.equal(await admin.locator("#approved-total").innerText(),"2.35");
      const downloadWorkbook = async (link, suffix) => {
        const downloaded = admin.waitForEvent("download");
        await link.click();
        const file = await downloaded;
        assert.equal(file.suggestedFilename(), "2025-2026学年综测申报汇总.xlsx");
        const filename = path.join(output, width + "-" + suffix + ".xlsx");
        await file.saveAs(filename);
        const parsed = spawnSync("python3", [path.join(__dirname, "support/read_xlsx.py"), filename], { encoding: "utf8" });
        assert.equal(parsed.status, 0, parsed.stderr);
        return JSON.parse(parsed.stdout);
      };
      const exported = await downloadWorkbook(admin.locator("#export-filtered"), "filtered");
      assert.deepEqual(exported.map(s => s.name), ["申报明细", "学生汇总"]);
      assert.equal(exported[0].rows.length, 2);
      assert.equal(exported[0].rows[1].B2.value, number);
      assert.equal(exported[0].rows[1].AD2.value, "已通过");
      assert.equal(exported[0].rows[1].AF2.value, 2.35);
      assert.equal(exported[1].rows[1].I2.value, 2.35);
      await checkWidth();
      await admin.screenshot({path:path.join(output,width+"-export.png"),fullPage:true});
      if (width < 640) {
        const button = await admin.locator("#export-filtered").boundingBox();
        const heading = await admin.locator("#list-heading").boundingBox();
        assert(button.y >= heading.y + heading.height, "Mobile export action should stack below its heading");
        assert.equal(Math.round(button.width), width - 32);
      }
      await admin.goto(base + "/admin?student_no=NO-MATCH-" + width);
      const empty = await downloadWorkbook(admin.locator("#export-filtered"), "empty");
      assert.equal(empty[0].rows.length, 1);
      assert.equal(empty[1].rows.length, 1);
      const current = await downloadWorkbook(admin.getByRole("link", {name:"导出当前学年", exact:true}), "current");
      assert(current[0].rows.length >= 2);

      await admin.goto(detailUrl);
      await admin.getByText("查看修改码", { exact: true }).click();
      assert.equal(await admin.locator("#admin-edit-code").innerText(), code);
      await admin.getByText("生成新修改码", { exact: true }).click();
      await admin.locator("#confirm-code-reset").check();
      const reset = admin.waitForResponse(response => response.url().endsWith("/edit-code/reset") && response.request().method() === "POST");
      await admin.getByRole("button", { name: "确认生成新修改码", exact: true }).click();
      assert.equal((await reset).status(), 303);
      await admin.waitForURL(url => url.searchParams.get("code_reset") === "1");
      const replacement = await admin.locator("#admin-edit-code").innerText();
      assert.notEqual(replacement, code);
      assert.equal((await studentContext.request.get(studentUrl)).status(), 403);
      assert.equal((await studentContext.request.get(base + attachmentUrl)).status(), 403);
      await student.goto(base + "/query");
      await student.locator("#submission-no").fill(number);
      await student.locator("#edit-code").fill(code);
      await student.getByRole("button", { name: "查询申报" }).click();
      await student.locator("#query-error").waitFor();
      await student.locator("#edit-code").fill(replacement);
      await student.getByRole("button", { name: "查询申报" }).click();
      await student.waitForURL(studentUrl);
      assert.equal(await student.locator(".status-badge").innerText(), "已通过");
      assert.equal((await studentContext.request.get(base + attachmentUrl)).status(), 200);
      assert.equal(await student.locator(".attachment-list li").count(), 2);

      await student.goto(base + "/submit");
      await student.locator("#student_name").fill("后台声明测试");
      await student.locator("#student_no").fill("ADMIN-NONE-" + width);
      await student.locator("#has-result-no").check();
      await student.locator("#no-result-confirm").check();
      await student.getByRole("button", { name: "提交申报", exact: true }).click();
      await student.waitForURL("**/success/declaration");
      await admin.goto(base + "/admin?student_no=ADMIN-NONE-" + width + "&status=approved");
      assert.equal(await admin.locator("#count-total").innerText(), "0");
      assert.equal(await admin.locator("#count-declarations").innerText(), "1");
      const declarationRow = admin.locator("tbody tr").filter({ hasText: "ADMIN-NONE-" + width });
      assert(await declarationRow.getByText("无申报材料", { exact: true }).isVisible());
      assert.equal(await declarationRow.getByRole("link").count(), 0);
      assert(!await declarationRow.innerText().then(text => /ZC\d{4}-\d{6}/.test(text)));
      await checkWidth();
      await admin.screenshot({ path: path.join(output, width + "-declaration-row.png"), fullPage: true });
      await admin.getByRole("button",{name:"退出登录"}).click();
      await admin.waitForURL("**/admin/login");
      assert.equal((await adminContext.request.get(base + attachmentUrl)).status(),403);
      assert.equal((await adminContext.request.get(base + "/admin/export.xlsx", {maxRedirects:0})).status(),303);
      await admin.goto(detailUrl);
      assert(admin.url().endsWith("/admin/login"));
      assert.deepEqual(errors,[]);
      await adminContext.close(); await studentContext.close();
      console.log("PASS admin workflow " + width + "x" + height + (width === 768 ? " (admin JavaScript disabled)" : ""));
    }
  } finally {
    if (browser) await browser.close();
    preview.kill("SIGINT");
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
