import type { Preview } from "@storybook/svelte-vite";
import "@lapismd/design-core/storybook.css";
import "@lapismd/design-core/themes/lapis.css";

const preview: Preview = {
  parameters: {
    layout: "fullscreen",
  },
};

export default preview;
