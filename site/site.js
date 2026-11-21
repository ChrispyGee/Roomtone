// Fills in the newest version and the right download links. Without JavaScript, or if GitHub
// can't be reached, every link still works: it points at the project's releases page.
(function () {
  var REPO = "ChrispyGee/Roomtone";
  var ua = navigator.userAgent || "";
  var os = /Windows/i.test(ua) ? "win" : /Macintosh|Mac OS X/i.test(ua) ? "mac" : /Linux|X11/i.test(ua) && !/Android/i.test(ua) ? "linux" : "other";
  document.documentElement.setAttribute("data-os", os);
  document.querySelectorAll("[data-platform]").forEach(function (el) {
    if (el.getAttribute("data-platform") === os) {
      el.classList.add("you");
      var h = el.querySelector("h3");
      if (h) { var t = document.createElement("span"); t.className = "you-tag"; t.textContent = "Your computer"; h.appendChild(t); }
    }
  });
  var patterns = { mac: /-macos-arm64\.dmg$/, win: /-windows-x64-setup\.exe$/, deb: /-linux-x64\.deb$/, rpm: /-linux-x64\.rpm$/, tgz: /-linux-x64\.tar\.gz$/, sums: /^SHA256SUMS$/ };
  fetch("https://api.github.com/repos/" + REPO + "/releases/latest", { headers: { Accept: "application/vnd.github+json" } })
    .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
    .then(function (rel) {
      var v = String(rel.tag_name || "").replace(/^v/, "");
      document.querySelectorAll("[data-version]").forEach(function (el) { el.textContent = v; });
      document.querySelectorAll("[data-tag]").forEach(function (el) { el.textContent = rel.tag_name; });
      (rel.assets || []).forEach(function (a) {
        Object.keys(patterns).forEach(function (k) {
          if (patterns[k].test(a.name)) {
            document.querySelectorAll('[data-dl="' + k + '"]').forEach(function (el) {
              el.href = a.browser_download_url;
              var n = el.querySelector("[data-file]"); if (n) n.textContent = a.name;
            });
            document.querySelectorAll('[data-fill="' + k + '"]').forEach(function (el) { el.textContent = a.name; });
          }
        });
      });
    })
    .catch(function () { /* keep the fallback links */ });
})();
