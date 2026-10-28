const screenshot = document.querySelector("#theme-screenshot");
const caption = document.querySelector("#theme-caption span");
let theme = "paper";
let paper = "plain";

document.querySelector(".theme-controls").hidden = false;

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
    });
  }
}
