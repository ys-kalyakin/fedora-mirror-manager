import "./style.css";
import { initTheme } from "./theme";
import { initShell } from "./shell";

// Theme first: the data-theme attribute is on the root before the shell is
// built, so a stored dark choice never flashes light at startup.
initTheme();
initShell();
