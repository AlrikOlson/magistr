export default {
  parameters: { layout: "fullscreen", controls: { disable: true }, viewport: {options: {
    desktop: {name:"Desktop · 1280 × 820",styles:{width:"1280px",height:"820px"}},
    compact: {name:"Compact · 760 × 550",styles:{width:"760px",height:"550px"}},
  }} },
  globalTypes: {
    appearance: { description: "Application appearance", toolbar: { icon: "mirror", items: ["dark", "light"], dynamicTitle: true } },
  },
  initialGlobals: { appearance: "dark", viewport: {value:"desktop",isRotated:false} },
};
