import { LEGACY_THEME_KEY } from "../lib/theme-key";

// Runs before the page paints, on the landing page. A visitor who chose a theme on the old site has it
// under "muxpilot-theme"; the generator's theme provider reads "theme". When only the old key exists,
// copy it over and stamp the class now (the provider's own script ran earlier and saw nothing).
export const THEME_MIGRATE = `(function(){try{var k=${JSON.stringify(LEGACY_THEME_KEY)};var t=localStorage.getItem(k);if((t==='light'||t==='dark')&&!localStorage.getItem('theme')){localStorage.setItem('theme',t);var r=document.documentElement;r.classList.remove('light','dark');r.classList.add(t);r.style.colorScheme=t;}}catch(e){}})();`;
