# 内置 Chromium（可选）

FinEnv 默认按以下顺序寻找 Chromium：

1. 环境级 `chromiumPath`
2. 全局设置 `chromiumPath`
3. 环境变量 `CHROME_PATH`
4. 本目录：
   - `chrome.exe`
   - `chromium.exe`
   - `chrome-win/chrome.exe`
   - `Chromium.app/Contents/MacOS/Chromium`
   - `chrome`
5. 系统已安装的 Chrome / Chromium / Edge

为实现「内核版本锁定」，建议将经过测试的 Chromium 便携版解压到本目录，并在「系统设置 → 浏览器内核」固定路径。仓库不包含浏览器二进制，避免体积与许可证问题。

内核升级前请先创建包含 profile 的加密备份；升级不会自动删除旧环境目录。
