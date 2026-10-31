import { beforeAll } from "vitest";
import { setProjectAnnotations } from "@storybook/svelte-vite";
import * as a11y from "@storybook/addon-a11y/preview";
import * as preview from "./preview";

const project = setProjectAnnotations([a11y, preview]);
beforeAll(project.beforeAll);
