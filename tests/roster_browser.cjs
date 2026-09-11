const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const { randomBytes } = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require(process.env.ZONGCE_PLAYWRIGHT_MODULE || "playwright");
const password = randomBytes(24).toString("hex");
const output = process.env.ZONGCE_SCREENSHOTS || "/tmp/zongce-roster-screenshots";
fs.mkdirSync(output, { recursive: true });
const preview = spawn(process.env.ZONGCE_PREVIEW_BIN || "target/debug/examples/admin_preview", [], {
  env: { ...process.env, ZONGCE_PREVIEW_PASSWORD: password }, stdio: ["ignore", "pipe", "pipe"],
});
const baseUrl = new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(new Error("Preview did not start")), 15000);
  let stdout = "";
  preview.stdout.on("data", chunk => {
    stdout += chunk;
    const match = stdout.match(/Admin preview: (http:\/\/127\.0\.0\.1:\d+)/);
    if (match) { clearTimeout(timeout); resolve(match[1]); }
  });
  preview.on("error", reject);
});

(async () => {
  let browser;
  try {
    const base = await baseUrl;
    browser = await chromium.launch({ channel: "chrome", headless: true });
    for (const [width, height] of [[320,568], [390,844], [768,1024], [1440,900]]) {
      const context = await browser.newContext({ viewport: {width,height}, javaScriptEnabled: width !== 768 });
      const studentContext = await browser.newContext({ viewport: {width,height} });
      const admin = await context.newPage(), student = await studentContext.newPage();
      const errors = [];
      admin.on("pageerror", e => errors.push(e.message));
      admin.setDefaultTimeout(8000); student.setDefaultTimeout(8000);
      const checked = async label => {
        assert(await admin.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "overflow " + label + " " + width);
        const ids = await admin.locator("[id]").evaluateAll(nodes => nodes.map(node => node.id));
        assert.equal(new Set(ids).size, ids.length, "duplicate ids");
        await admin.screenshot({ path: path.join(output, width + "-" + label + ".png"), fullPage: true });
      };
      await admin.goto(base + "/admin/login");
      await admin.locator("#username").fill("preview-admin");
      await admin.locator("#password").fill(password);
      await admin.getByRole("button", {name:"登录",exact:true}).click();
      await admin.waitForURL(base + "/admin");
      await admin.goto(base + "/admin/settings");
      assert.equal(await admin.locator("#year-name").count(), 0, "year editor must follow roster import");
      const template = await context.request.get(base + "/admin/roster/template.xlsx");
      assert.equal(template.status(), 200);
      assert((await template.body()).subarray(0,2).equals(Buffer.from("PK")));
      await admin.locator("#roster_file").setInputFiles({ name:"名单.csv", mimeType:"text/csv", buffer:Buffer.from("\ufeff姓名,学号\n名单测试学生,000001\n名单测试学生,000001") });
      await admin.getByRole("button", {name:"校验名单，继续创建学年",exact:true}).click();
      await admin.waitForURL(url => url.searchParams.get("saved") === "roster");
      assert((await admin.locator("main").innerText()).includes("已准备 1 名学生"));
      await checked("create");
      const yearName = "名单浏览器测试-" + width;
      await admin.locator("#year-name").fill(yearName);
      await admin.locator("#year-start_date").fill("2026-08-31");
      await admin.locator("#year-end_date").fill("2027-08-30");
      await admin.getByRole("button", {name:"创建学年",exact:true}).click();
      await admin.waitForURL(url => url.searchParams.get("saved") === "year");
      const row = admin.locator("tbody tr").filter({has:admin.getByRole("cell",{name:yearName,exact:true})});
      const rosterUrl = await row.getByRole("link", {name:"1 人 · 管理名单"}).getAttribute("href");
      const yearId = rosterUrl.match(/years\/(\d+)/)[1];
      await row.getByRole("button", {name:"激活" + yearName,exact:true}).click();
      await admin.waitForURL(url => url.searchParams.get("saved") === "active");
      await admin.goto(base + rosterUrl);
      await admin.locator("#roster_text").fill("姓名\t学号\n补录测试学生\t000002");
      await admin.getByRole("button", {name:"添加学生",exact:true}).click();
      await admin.waitForURL(url => url.searchParams.get("saved") === "1");
      assert((await admin.locator("main").innerText()).includes("2 名学生"));
      await checked("roster");
      await student.goto(base);
      await student.locator("#access-code").fill("preview-only");
      await student.getByRole("button", {name:"继续填写",exact:true}).click();
      await student.waitForURL("**/submit");
      await student.locator("#student_name").fill("补录测试学生");
      await student.locator("#student_no").fill("not-in-roster");
      await student.locator("#has-result-no").check();
      await student.locator("#no-result-confirm").check();
      await student.getByRole("button", {name:"提交申报",exact:true}).click();
      await student.getByText("姓名与学号不在本学年允许申报名单中", {exact:false}).first().waitFor();
      await student.locator("#student_no").fill("000002");
      await student.getByRole("button", {name:"提交申报",exact:true}).click();
      await student.waitForURL("**/success/declaration");
      await admin.goto(base + "/admin");
      await admin.getByRole("button", {name:"删除",exact:true}).click();
      await admin.getByRole("heading", {name:"确认删除申报",exact:true}).waitFor();
      await checked("delete-confirm");
      await admin.locator('[name="confirm_delete"]').check();
      await admin.getByRole("button", {name:"确认删除",exact:true}).click();
      await admin.waitForURL(url => url.pathname === "/admin/recycle");
      await checked("recycle");
      await admin.locator('input[type="checkbox"]').check();
      await admin.getByRole("button", {name:"恢复所选记录",exact:true}).click();
      await admin.waitForURL(url => url.pathname === "/admin/recycle");
      assert((await admin.locator("main").innerText()).includes("回收站暂无记录"));
      await admin.goto(base + "/admin");
      assert.equal(await admin.locator("#count-declarations").innerText(), "1");
      await admin.goto(base + "/admin/years/" + yearId + "/delete");
      await checked("delete-year");
      await admin.locator("#confirm_name").fill(yearName);
      await admin.locator('[name="confirm_delete"]').check();
      await admin.getByRole("button", {name:"永久删除学年",exact:true}).click();
      await admin.waitForURL(url => url.searchParams.get("saved") === "deleted");
      assert.equal(await admin.getByRole("cell", {name:yearName,exact:true}).count(), 0);
      assert.equal(await admin.getByRole("cell", {name:"2025-2026学年",exact:true}).count(), 1);
      assert.deepEqual(errors, []);
      await context.close(); await studentContext.close();
      console.log(width + "x" + height + ": roster import, create, append, eligibility, recycle/restore and year deletion passed");
    }
  } finally {
    if (browser) await browser.close();
    preview.kill("SIGINT");
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
