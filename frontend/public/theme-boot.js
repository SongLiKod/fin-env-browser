(function () {
  var KEY = 'finenv-theme';
  var DEFAULT = { mode: 'auto', primaryColor: '#165DFF', density: 'comfortable' };
  var theme = DEFAULT;
  try {
    var raw = localStorage.getItem(KEY);
    if (raw) {
      var parsed = JSON.parse(raw);
      theme = Object.assign({}, DEFAULT, parsed);
    }
  } catch (e) {}
  function hexToRgb(hex) {
    var h = String(hex || '').replace('#', '');
    if (h.length !== 6 || /[^0-9a-f]/i.test(h)) h = '165DFF';
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  }
  function mix(hex, target, amount) {
    var rgb = hexToRgb(hex);
    var out = rgb.map(function (c, i) {
      return Math.round(c + (target[i] - c) * amount);
    });
    return '#' + out.map(function (c) { return c.toString(16).padStart(2, '0'); }).join('');
  }
  var resolved = theme.mode;
  if (resolved === 'auto') {
    resolved = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  var root = document.documentElement;
  root.setAttribute('data-theme', resolved);
  root.setAttribute('data-density', theme.density || 'comfortable');
  root.style.setProperty('--color-primary', theme.primaryColor);
  root.style.setProperty('--color-primary-light', mix(theme.primaryColor, [255, 255, 255], 0.9));
  root.style.setProperty('--color-primary-dark', mix(theme.primaryColor, [0, 0, 0], 0.16));
  var rgb = hexToRgb(theme.primaryColor);
  var lum = (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 255;
  root.style.setProperty('--text-on-primary', lum > 0.62 ? '#1d2129' : '#ffffff');
  root.style.colorScheme = resolved;
})();
