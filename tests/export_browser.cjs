// Run the existing complete student workflow, then export its real seven-category records.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const { randomBytes } = require("node:crypto");
const { chromium } = require(process.env.ZONGCE_PLAYWRIGHT_MODULE || "playwright");
const password = randomBytes(24).toString("hex");
const output = process.env.ZONGCE_SCREENSHOTS || "/tmp/zongce-export-screenshots";
fs.mkdirSync(output, {recursive:true});
const preview = spawn(process.env.ZONGCE_PREVIEW_BIN || "target/debug/examples/admin_preview", [], {
  env:{...process.env,ZONGCE_PREVIEW_PASSWORD:password}, stdio:["ignore","pipe","pipe"],
});
const baseUrl = new Promise((resolve,reject) => {
  const timeout=setTimeout(()=>reject(new Error("Preview did not start")),15000);
  let stdout="";
  preview.stdout.on("data",chunk=>{
    stdout+=chunk;
    const match=stdout.match(/Admin preview: (http:\/\/127\.0\.0\.1:\d+)/);
    if(match){clearTimeout(timeout);resolve(match[1]);}
  });
  preview.on("error",error=>{clearTimeout(timeout);reject(error);});
  preview.on("exit",code=>{clearTimeout(timeout);reject(new Error("Preview exited: "+code));});
});

(async()=>{
  let browser;
  try {
    const base=await baseUrl;
    await new Promise((resolve,reject)=>{
      const regression=spawn(process.execPath,[path.join(__dirname,"student_browser.cjs"),base],{
        env:{...process.env,ZONGCE_SCREENSHOTS:output},stdio:"inherit",
      });
      regression.on("error",reject);
      regression.on("exit",code=>code===0?resolve():reject(new Error("Student regression failed: "+code)));
    });
    browser=await chromium.launch({channel:"chrome",headless:true});
    const page=await browser.newPage({viewport:{width:1440,height:900}});
    await page.goto(base+"/admin/login");
    await page.locator("#username").fill("preview-admin");
    await page.locator("#password").fill(password);
    await page.getByRole("button",{name:"登录",exact:true}).click();
    await page.waitForURL(base+"/admin");
    assert.equal(await page.locator("#count-total").innerText(),"22");
    const downloaded=page.waitForEvent("download");
    await page.getByRole("link",{name:"导出当前学年",exact:true}).click();
    const file=await downloaded;
    assert.equal(file.suggestedFilename(),"2025-2026学年综测申报汇总.xlsx");
    const filename=path.join(output,"seven-categories.xlsx");
    await file.saveAs(filename);
    const parsed=spawnSync("python3",[path.join(__dirname,"support/read_xlsx.py"),filename],{encoding:"utf8",maxBuffer:4*1024*1024});
    assert.equal(parsed.status,0,parsed.stderr);
    const [detail,summary]=JSON.parse(parsed.stdout);
    assert.equal(detail.rows.length,23);
    assert.equal(summary.rows.length,9);
    const categories=new Set(detail.rows.slice(1).map((row,index)=>row["F"+(index+2)].value));
    assert.deepEqual([...categories].sort(),["学术科技类竞赛","文体类比赛","其他获奖表彰","发表文章","社会实践/服务","专利","学习技能/资格证书"].sort());
    for(let index=1;index<detail.rows.length;index++){
      const row=detail.rows[index], number=index+1;
      assert.equal(row["AB"+number].value,1);
      assert.equal(row["AD"+number].value,"待审核");
      assert.equal(row["G"+number].value,"浏览器修改已保存");
      assert.equal(row["H"+number].type,"n");
      if (row["V"+number]?.value === "CET-4/CET-6") {
        assert.equal(row["W"+number].type,"n");
        assert([500,525].includes(row["W"+number].value));
      }
    }
    for(let index=1;index<summary.rows.length;index++) assert.equal(summary.rows[index]["I"+(index+1)].value,0);
    await page.screenshot({path:path.join(output,"1440-seven-categories.png"),fullPage:true});
    console.log("PASS seven-category XLSX from actual browser submissions, edits and declarations: "+filename);
  } finally {
    if(browser) await browser.close();
    preview.kill("SIGINT");
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
