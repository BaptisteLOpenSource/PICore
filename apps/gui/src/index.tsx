import "virtual:uno.css";
import { render } from "solid-js/web";
import { App } from "./App";
import { start } from "./store";

start();
render(() => <App />, document.getElementById("root")!);
