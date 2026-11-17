// Fixture-driven, real-browser check. Start this worktree's Vite server, open
// its localhost URL in playwright-cli, then:
// playwright-cli run-code --filename=scripts/check-pr-content-loading.js
// All API replies are injected; remote requests and provider writes are blocked.
// This proves browser presentation behaviour, not native WebView behaviour or
// real DNS/TLS (the Rust resolver/connector tests cover that boundary).
// eslint-disable-next-line no-unused-expressions -- Playwright consumes the function as its entry point.
async page => {
    const origin = new URL(page.url()).origin
    if (!/^http:\/\/localhost:\d+$/.test(origin)) { throw Error("Open this worktree's localhost Vite URL first") }
    const errors = []
    const privateLogs = []
    const consoleListener = message => {
        if (message.text().includes("fixture-secret") || message.text().includes("iVBORw0KGgo")) { privateLogs.push("private diagnostic") }
    }
    page.on("console", consoleListener)
    await page.route("https://**/*", route => { errors.push("unexpected remote request"); return route.abort() })
    await page.route("**/api/**", route => route.abort())
    await page.route("**/__pr_content_loading", route => route.fulfill({
        contentType: "text/html",
        body: String.raw`<!doctype html><html data-theme="forge" data-scheme="light"><head>
<meta charset="utf-8"><title>PR loading fixture</title>
<link rel="stylesheet" href="/src/fonts.css"><link rel="stylesheet" href="/src/styles/base.css">
<link rel="stylesheet" href="/src/styles/themes/forge.css"><link rel="stylesheet" href="/src/styles/themes/blueprint.css">
<link rel="stylesheet" href="/src/styles/themes/folio.css"><link rel="stylesheet" href="/src/styles/themes/mycelium.css">
</head><body><div id="smoke" class="split-pane-right" style="height:100vh;overflow:auto"></div>
<script type="module">
import RefreshRuntime from "/@react-refresh";
RefreshRuntime.injectIntoGlobalHook(window);
window.$RefreshReg$=()=>{};window.$RefreshSig$=()=>type=>type;
window.__vite_plugin_react_preamble_installed__=true;
const React=(await import("/node_modules/.vite/deps/react.js")).default;
const ReactDOM=(await import("/node_modules/.vite/deps/react-dom_client.js")).default;
const {PullRequestView}=await import("/src/components/PullRequestView.tsx");
const {applyAppearance}=await import("/src/appearance.ts");
const {workspaceKey,writeWorkspace}=await import("/src/pullRequestWorkspace.ts");
const {sha256Hex}=await import("/src/sha256.ts");
const png=(()=>{
 const canvas=document.createElement("canvas");canvas.width=1200;canvas.height=800;
 const context=canvas.getContext("2d");
 context.fillStyle="#253745";context.fillRect(0,0,1200,800);
 context.fillStyle="#79c6cc";context.fillRect(60,60,1080,120);
 context.fillStyle="#e1e8ee";context.font="40px sans-serif";context.fillText("Published PR screenshot fixture",60,260);
 return canvas.toDataURL("image/png").split(",")[1];
})();
const f=window.fixture={sent:[],events:new Map(),active:0,peak:0,generation:0,started:0,completed:0,fail:{},appearance:applyAppearance};
f.emit=(event,payload)=>{for(const callback of f.events.get(event)??[])callback({data:JSON.stringify(payload)});};
window.EventSource=class{
 readyState=1;
 addEventListener(type,callback){let set=f.events.get(type);if(!set)f.events.set(type,set=new Set());set.add(callback);}
 removeEventListener(type,callback){f.events.get(type)?.delete(callback);}
 close(){}
};
const hunk=()=>({oldStart:1,oldLines:8,newStart:1,newLines:8,section:"fixture",
 lines:Array.from({length:f.big?120:8},(_,i)=>({kind:"context",oldNo:i+1,newNo:i+1,
 text:"                                const fixture"+i+" = 'A deliberately long unchanged line for synchronized source scrolling and retained selection';"}))});
const file=(path,kind)=>({oldPath:path,newPath:path,oldMode:"100644",newMode:"100644",status:{kind:"modified"},
 additions:8,deletions:0,content:kind==="hunks"?{kind,hunks:[hunk()]}:{kind}});
const comment=(id,body,minimizedReason=null)=>({id,author:"reviewer",body,postedAtUnix:0,url:null,minimizedReason,deleted:false});
const answer=async(command,args)=>{
 f.sent.push({command,args});
 if(command==="get_pr_content_loading")return f.preferences;
 if(command==="set_pr_content_loading"){f.preferences={...f.preferences,[args.preference]:args.enabled};f.emit("pr-content-loading-changed",f.preferences);return f.preferences;}
 if(command==="get_pull_request_detail")return {kind:"detail",detail:f.detail};
 if(command==="get_review_progress")return f.progress;
 if(command==="get_pull_request_file"||command==="get_pull_request_file_image"||command==="get_pull_request_content_image"){
  const generation=f.generation;
  f.active++;f.started++;f.peak=Math.max(f.peak,f.active);
  try{
   await new Promise(resolve=>setTimeout(resolve,f.delay??35));
   if(generation!==f.generation)return {kind:command==="get_pull_request_content_image"?"stale":"changed"};
   if(command==="get_pull_request_content_image"){
    const bodies=[f.detail.description,...f.detail.conversation.map(e=>e.comment.body),...f.detail.threads.flatMap(t=>t.comments.map(c=>c.body))];
    if(!bodies.some(body=>sha256Hex(body)===args.bodyDigest&&body.includes(args.source.split("#")[0])))return {kind:"stale"};
    if(args.source.includes("blocked"))return {kind:"failed",reason:"unsafeAddress",untilUnix:null};
    if(args.source.includes("protected"))return {kind:"failed",reason:"unavailable",untilUnix:null};
    if(f.fail.image){f.fail.image=false;return {kind:"failed",reason:"transient",untilUnix:null};}
    return {kind:"image",mime:"image/png",width:1200,height:800,data:png,finalHost:"cdn.example"};
   }
   if(args.head!==f.detail.headCommit||args.base!==f.detail.baseCommit)return {kind:"changed"};
   if(f.fail[args.path]){f.fail[args.path]=false;return {kind:"failed",reason:"deferred",untilUnix:2000000000};}
   if(command==="get_pull_request_file")return {kind:"file",file:file(args.path,"hunks")};
   return {kind:"images",old:{kind:"absent"},new:{kind:"image",mime:"image/png",width:1200,height:800,data:png}};
  }finally{f.completed++;if(generation===f.generation)f.active--;}
 }
 throw Error("Unexpected fixture command: "+command);
};
window.fetch=async(url,options)=>{
 if(url!=="/api/invoke")throw Error("Unexpected fixture fetch");
 const {command,args}=JSON.parse(options.body);
 return new Response(JSON.stringify(await answer(command,args)),{headers:{"Content-Type":"application/json"}});
};
f.root=ReactDOM.createRoot(document.getElementById("smoke"));
f.reset=(provider,preferences,surface="main")=>{
 f.generation++;f.sent=[];f.active=0;f.peak=0;f.fail={};f.preferences=preferences;f.big=false;f.delay=35;
 f.reference={provider,owner:"fixture",repo:"loading",number:9900+f.generation};
 const r=f.reference;
 const url=provider==="github"?"https://github.com/fixture/loading/pull/"+r.number:"https://bitbucket.org/fixture/loading/pull-requests/"+r.number;
 f.row={id:r.number,title:"Automatic loading fixture",repoFullName:"fixture/loading",sourceRepoFullName:"fixture/loading",
 sourceBranch:"feature",destinationBranch:"master",url,draft:false,updatedAtUnix:1,review:null,openTasks:0,author:"reviewer",
 checks:null,conflicting:false,unresolvedThreads:1};
 const general=comment("general","General ![general](https://images.example/general.png?sig=fixture-secret)");
 f.detail={reference:r,row:f.row,headBranch:"feature",baseBranch:"master",headCommit:"head",baseCommit:"base",author:"reviewer",
 description:"![screen](https://images.example/screen.png?sig=fixture-secret)\n\n[![linked](https://images.example/screen.png?sig=fixture-secret#again)](https://example.com/details)\n\n![blocked](https://blocked.example/a.png)\n\n![protected](https://protected.example/a.png)\n\n<img src=\"https://images.example/raw.png\">\n\n![relative](./relative.png)",
 conversation:[{review:null,comment:general},{review:null,comment:comment("minimized","![hidden](https://images.example/minimized.png)","spam")}],
 checks:[],threads:[{id:"inline",path:"stable.ts",side:"new",line:3,startSide:null,startLine:null,originalLine:null,originalStartLine:null,
 resolved:false,outdated:false,replyable:true,comments:[comment("inline-comment","Inline ![inline](https://images.example/inline.png?sig=fixture-secret)")]}],
 files:[file("cached.ts","withheld"),file("patchless.ts","withheld"),file("image.png","binary"),file("skipped.ts","withheld"),file("stable.ts","hunks")],
 unlistedFiles:0,readAtUnix:1,noLongerListed:false};
 f.progress={files:f.detail.files.map(file=>({path:file.newPath,state:file.newPath==="skipped.ts"?"skipped":"unviewed",
 keyedByHead:false,hunks:null,matched:file.newPath==="skipped.ts"?"skipped.*":null,included:false})),
 viewed:0,changedSinceViewed:0,skipped:1,total:5,lastMarkedHead:null,headCommit:"head",baseCommit:"base"};
 document.body.dataset.surface=surface==="detached"?"pull-request":"main";
 writeWorkspace(workspaceKey(r,surface),{tab:"overview",place:null});
 document.getElementById("smoke").scrollTop=0;
 f.root.render(React.createElement(React.StrictMode,null,React.createElement(PullRequestView,{
  key:f.generation,address:{kind:"pullRequest",...r},row:f.row,views:[],links:null,
  pane:surface==="main"?{openWorktree(){},openSettings(){}}:undefined
 })));
};
f.toggle=(preference,enabled)=>{f.preferences={...f.preferences,[preference]:enabled};f.emit("pr-content-loading-changed",f.preferences);};
f.reset("github",{autoShowImages:true,autoLoadLargeDiffs:true});
</script></body></html>`,
    }))
    const assert = (value, name) => { if (!value) { throw Error(`PR loading regression: ${name}`) } }
    const settle = async () => {
        await page.evaluate(async () => { await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame) })
        await page.waitForFunction(() => window.fixture.active === 0)
        await page.evaluate(async () => { await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame) })
    }
    const calls = command => page.evaluate(command => window.fixture.sent.filter(item => item.command === command).length, command)
    try {
        await page.goto(`${origin}/__pr_content_loading`)
        await page.getByRole("tab", { name: "Overview", exact: true }).waitFor()
        await settle()
        let cases = 0, maximumObserved = 0
        for (const provider of ["github", "bitbucket"]) {
            for (const surface of ["main", "detached"]) {
                for (const autoShowImages of [false, true]) {
                    for (const autoLoadLargeDiffs of [false, true]) {
                        for (const width of [1560, 480]) {
                            await page.setViewportSize({ width, height: 1000 })
                            await page.evaluate(options => window.fixture.reset(...options),
                                [provider, { autoShowImages, autoLoadLargeDiffs }, surface])
                            await settle()
                            if (autoShowImages) {
                                await page.waitForFunction(() => window.fixture.sent.filter(item =>
                                    item.command === "get_pull_request_content_image").length === 3 && window.fixture.active === 0)
                            }
                            assert(await calls("get_pull_request_file") === 0 && await calls("get_pull_request_file_image") === 0, "hidden Files fetched")
                            const descriptionReads = await calls("get_pull_request_content_image")
                            assert(autoShowImages ? descriptionReads === 3 : descriptionReads === 0, "description source deduplication or preference")
                            await page.getByRole("tab", { name: /^Files/ }).click()
                            for (const path of ["cached.ts", "patchless.ts", "image.png", "stable.ts"]) {
                                await page.locator(`[data-file-key="${path}"]`).scrollIntoViewIfNeeded()
                                await settle()
                                if (autoLoadLargeDiffs && path.endsWith(".ts") && path !== "stable.ts") {
                                    await page.waitForFunction(path => window.fixture.sent.some(item =>
                                        item.command === "get_pull_request_file" && item.args.path === path) && window.fixture.active === 0, path)
                                }
                                if (autoShowImages && path === "image.png") {
                                    await page.waitForFunction(() => window.fixture.sent.some(item =>
                                        item.command === "get_pull_request_file_image") && window.fixture.active === 0)
                                }
                            }
                            assert(await calls("get_pull_request_file") === (autoLoadLargeDiffs ? 2 : 0), "withheld loading or skipped intent")
                            assert(await calls("get_pull_request_file_image") === Number(autoShowImages), "changed-image loading")
                            assert(await page.locator('[data-file-key="skipped.ts"] .diff-file-toggle').getAttribute("aria-expanded") === "false", "skipped section expanded")
                            const peak = await page.evaluate(() => window.fixture.peak)
                            maximumObserved = Math.max(maximumObserved, peak)
                            assert(peak <= 2, "mixed concurrency")
                            if (autoShowImages) {
                                await page.waitForFunction(() => window.fixture.sent.some(item =>
                                    item.command === "get_pull_request_content_image" && item.args.source.includes("/inline.png")))
                            }
                            const before = await calls("get_pull_request_content_image")
                            await page.getByRole("tab", { name: /^Discussion/ }).click()
                            await settle()
                            assert(await calls("get_pull_request_content_image") === before + Number(autoShowImages), "general comment preview")
                            assert(await page.locator("#pr-panel-discussion img").count() === Number(autoShowImages), "minimized comment fetched")
                            const metadata = await page.evaluate(() => ({
                                remote: [...document.querySelectorAll("img")].some(image => image.src.startsWith("https:") || image.hasAttribute("srcset")),
                                nested: document.querySelector("a a, a button") !== null,
                                overflow: [...document.querySelectorAll(".pr-content-image-preview")].some(image => image.getBoundingClientRect().width > image.closest(".markdown-view").getBoundingClientRect().width + 1),
                                writes: window.fixture.sent.some(item => /^(submit|create_review|edit_review|set_file|set_hunk)/.test(item.command)),
                            }))
                            assert(!metadata.remote && !metadata.nested && !metadata.overflow && !metadata.writes, "untrusted-content boundary or containment")
                            cases++
                        }
                    }
                }
            }
        }
        // Retention and retry are explicit, never driven by a deadline timer.
        await page.evaluate(() => { window.fixture.reset("github", { autoShowImages: true, autoLoadLargeDiffs: true }); window.fixture.fail.image = true })
        await settle()
        const retry = page.getByRole("button", { name: "Try again", exact: true }).first()
        await retry.waitFor()
        const coarse = await page.evaluate(() => matchMedia("(pointer: coarse)").matches)
        if (coarse) { assert(await retry.evaluate(element => element.offsetHeight >= 44 && element.offsetWidth >= 44), "coarse retry target") }
        const reads = await calls("get_pull_request_content_image")
        await page.evaluate(() => { window.fixture.toggle("autoShowImages", false); window.fixture.toggle("autoShowImages", true) })
        await settle()
        assert(await calls("get_pull_request_content_image") === reads, "implicit retry after off/on")
        await retry.click()
        await settle()
        assert(await calls("get_pull_request_content_image") === reads + 1, "explicit retry")
        const loaded = await page.locator("#pr-panel-overview img").count()
        await page.evaluate(() => window.fixture.toggle("autoShowImages", false))
        await settle()
        assert(await page.locator("#pr-panel-overview img").count() === loaded, "loaded preview discarded")
        let geometryCases = 0
        for (const surface of ["main", "detached"]) {
        await page.evaluate(surface => window.fixture.reset("github", { autoShowImages: true, autoLoadLargeDiffs: true }, surface), surface)
        await page.waitForFunction(() => window.fixture.sent.filter(item => item.command === "get_pull_request_content_image").length === 3 && window.fixture.active === 0)
        for (const theme of ["forge", "blueprint", "folio", "mycelium"]) {
            for (const scheme of ["light", "dark"]) {
                await page.evaluate(value => window.fixture.appearance(value), { theme, scheme })
                for (const width of [1560, 480]) {
                    await page.setViewportSize({ width, height: 1000 })
                    await page.evaluate(() => { document.documentElement.style.zoom = "1.5" })
                    await settle()
                    assert(await page.locator("a a, a button").count() === 0, "nested link at enlarged zoom")
                    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), "horizontal page overflow")
                    geometryCases++
                }
            }
        }
        }
        await page.evaluate(() => { document.documentElement.style.zoom = ""; window.fixture.toggle("autoShowImages", true) })
        // Minimized comments start only after the native details element opens.
        await page.getByRole("tab", { name: /^Discussion/ }).click()
        await settle()
        const minimizedBefore = await calls("get_pull_request_content_image")
        await page.locator("#pr-panel-discussion details summary").click()
        await page.waitForFunction(() => window.fixture.sent.some(item =>
            item.command === "get_pull_request_content_image" && item.args.source.includes("/minimized.png")))
        await settle()
        assert(await calls("get_pull_request_content_image") === minimizedBefore + 1, "minimized expansion")

        // Leaving a retained Files tab must prevent its waiting work from starting.
        await page.evaluate(() => {
            const f = window.fixture
            f.reset("github", { autoShowImages: false, autoLoadLargeDiffs: true })
            f.delay = 250
            f.detail.files.splice(2, 0, { ...f.detail.files[1], oldPath: "waiting.ts", newPath: "waiting.ts" })
            f.progress.files.push({ ...f.progress.files[1], path: "waiting.ts" })
            f.progress.total++
        })
        await settle()
        await page.getByRole("tab", { name: /^Files/ }).click()
        await page.waitForFunction(() => window.fixture.active === 2)
        await page.getByRole("tab", { name: /^Discussion/ }).click()
        const hiddenBefore = await calls("get_pull_request_file")
        await settle()
        assert(hiddenBefore === 2, "hidden-queue fixture did not fill both slots")
        assert(await calls("get_pull_request_file") === hiddenBefore, "hidden queue pumped after completion")
        await page.getByRole("tab", { name: /^Files/ }).click()
        await page.locator('[data-file-key="waiting.ts"]').scrollIntoViewIfNeeded()
        await page.waitForFunction(() => window.fixture.sent.some(item =>
            item.command === "get_pull_request_file" && item.args.path === "waiting.ts"))
        await settle()

        await page.evaluate(() => { window.fixture.reset("github", { autoShowImages: false, autoLoadLargeDiffs: true }); window.fixture.fail["patchless.ts"] = true })
        await settle()
        await page.getByRole("tab", { name: /^Files/ }).click()
        await page.waitForFunction(() => window.fixture.sent.filter(item => item.command === "get_pull_request_file").length === 2 && window.fixture.active === 0)
        const withheld = page.locator('[data-file-key="patchless.ts"]')
        await withheld.getByRole("button", { name: "Load diff", exact: true }).waitFor()
        const deferredReads = await calls("get_pull_request_file")
        await page.evaluate(() => { window.fixture.toggle("autoLoadLargeDiffs", false); window.fixture.toggle("autoLoadLargeDiffs", true) })
        await settle()
        assert(await calls("get_pull_request_file") === deferredReads, "automatic retry of a provider deferral")
        await withheld.getByRole("button", { name: "Load diff", exact: true }).click()
        await settle()
        assert(await calls("get_pull_request_file") === deferredReads + 1, "manual deferred-file retry")

        // Retain a real DOM selection while large text is inserted above it.
        await page.evaluate(() => window.fixture.reset("github", { autoShowImages: false, autoLoadLargeDiffs: false }))
        await settle()
        await page.getByRole("tab", { name: /^Files/ }).click()
        await page.locator('[data-file-key="stable.ts"] .diff-text').first().waitFor()
        const anchorBefore = await page.evaluate(() => {
            const cell = document.querySelector('[data-file-key="stable.ts"] .diff-text')
            const range = document.createRange()
            const text = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT).nextNode()
            range.setStart(text, 0); range.setEnd(text, Math.min(text.textContent.length, 4))
            getSelection().removeAllRanges(); getSelection().addRange(range)
            window.fixture.anchorCell = cell
            window.fixture.big = true
            const result = { top: cell.getBoundingClientRect().top, selection: getSelection().toString() }
            window.fixture.toggle("autoLoadLargeDiffs", true)
            return result
        })
        await page.waitForFunction(() => window.fixture.sent.filter(item => item.command === "get_pull_request_file").length === 2 && window.fixture.active === 0)
        await settle()
        const anchorAfter = await page.evaluate(() => ({
            top: fixture.anchorCell.getBoundingClientRect().top, selection: getSelection().toString(),
            same: fixture.anchorCell === document.querySelector('[data-file-key="stable.ts"] .diff-text'),
        }))
        assert(anchorAfter.same && anchorAfter.selection === anchorBefore.selection && Math.abs(anchorAfter.top - anchorBefore.top) < 2,
            "text insertion moved selection or reading anchor")

        // Old-generation completions cannot populate the replacement PR.
        await page.evaluate(() => { window.fixture.reset("github", { autoShowImages: true, autoLoadLargeDiffs: false }); window.fixture.delay = 200 })
        await page.waitForFunction(() => window.fixture.active > 0)
        await page.evaluate(() => window.fixture.reset("bitbucket", { autoShowImages: false, autoLoadLargeDiffs: false }))
        await page.waitForFunction(() => window.fixture.completed === window.fixture.started)
        await settle()
        assert(await page.locator("img").count() === 0, "old source bytes reached replacement PR")
        assert(!privateLogs.length && !errors.length, "source-query logging or remote decoder fetch")
        return { policyCases: cases, geometryCases, coarsePointer: coarse, mixedConcurrency: maximumObserved, selectionAndScrollRetained: true,
            hiddenQueuePaused: true, oldResultsFenced: true, privateLogEntries: privateLogs.length, remoteRequests: errors.length }
    } finally {
        page.off("console", consoleListener)
    }
}
