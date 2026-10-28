const preview = document.querySelector("#paper-preview");
const caption = document.querySelector("#preview-caption");

document.querySelector(".sampler-controls").hidden = false;

for (const group of ["theme", "paper"]) {
  const buttons = document.querySelectorAll(`button[data-${group}]`);

  for (const button of buttons) {
    button.addEventListener("click", () => {
      preview.dataset[group] = button.dataset[group];

      for (const sibling of buttons) {
        const selected = sibling === button;
        sibling.setAttribute("aria-pressed", String(selected));
        sibling.classList.toggle("active", selected);
      }

      caption.textContent =
        `${preview.dataset.theme} + ${preview.dataset.paper} / A LIVE WEBSITE PREVIEW`.toUpperCase();
    });
  }
}
