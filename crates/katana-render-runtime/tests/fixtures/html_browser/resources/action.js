const pixel = document.querySelector("#pixel");
if (!(pixel instanceof HTMLElement)) throw new TypeError("pixel must be an HTML element");
pixel.style.background = "rgb(17, 34, 51)";
pixel.addEventListener("click", () => {
  pixel.style.background = "rgb(119, 136, 153)";
});
