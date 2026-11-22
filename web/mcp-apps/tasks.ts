import { mount } from "svelte";
import TasksView from "./TasksView.svelte";
import "./styles.css";
import "@lapis-notes/lapis-plugin-tasks/styles.css";

mount(TasksView, { target: document.body.firstElementChild! });
