import { mount } from "svelte";
import NoteView from "./NoteView.svelte";
import "./styles.css";
import "@lapismd/mira/preview/styles.css";

mount(NoteView, { target: document.body.firstElementChild! });
