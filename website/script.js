const screenshot = document.querySelector("#theme-screenshot");
const caption = document.querySelector("#theme-caption span");
const stage = document.querySelector(".theme-stage");
const leftNeighbor = document.querySelector("#theme-neighbor-left");
const rightNeighbor = document.querySelector("#theme-neighbor-right");
const themeButtons = [...document.querySelectorAll("button[data-theme]")];
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
let theme = "paper";
let paper = "plain";

document.querySelector(".theme-controls").hidden = false;

screenshot.addEventListener("load", () => {
  if (!reducedMotion.matches) {
    screenshot.animate([{ opacity: 0.4 }, { opacity: 1 }], {
      duration: 240,
      easing: "ease-out",
    });
  }
});

for (const group of ["theme", "paper"]) {
  const buttons = document.querySelectorAll(`button[data-${group}]`);

  for (const button of buttons) {
    button.addEventListener("click", () => {
      if (group === "theme") theme = button.dataset.theme;
      else paper = button.dataset.paper;

      for (const sibling of buttons) {
        const selected = sibling === button;
        sibling.setAttribute("aria-pressed", String(selected));
        sibling.classList.toggle("active", selected);
      }

      const themeName = document
        .querySelector(`button[data-theme="${theme}"]`)
        .lastElementChild.textContent.trim();
      const paperName = document
        .querySelector(`button[data-paper="${paper}"]`)
        .textContent.trim();
      screenshot.src = `assets/themes/${theme}-${paper}.png`;
      screenshot.alt = `Bulletnotes editor in the ${themeName} theme with ${paperName} paper, showing the sample Scratchpad note.`;
      caption.textContent = `${themeName} · ${paperName}`;

      const index = themeButtons.findIndex(
        (item) => item.dataset.theme === theme,
      );
      const previous =
        themeButtons[(index + themeButtons.length - 1) % themeButtons.length];
      const next = themeButtons[(index + 1) % themeButtons.length];
      leftNeighbor.src = `assets/themes/${previous.dataset.theme}-${paper}.png`;
      leftNeighbor.alt = `Bulletnotes in the ${previous.lastElementChild.textContent.trim()} theme.`;
      rightNeighbor.src = `assets/themes/${next.dataset.theme}-${paper}.png`;
      rightNeighbor.alt = `Bulletnotes in the ${next.lastElementChild.textContent.trim()} theme.`;
      stage.style.setProperty(
        "--stage-glow",
        getComputedStyle(
          themeButtons[index].querySelector(".theme-chip"),
        ).getPropertyValue("--chip-accent"),
      );
    });
  }
}
