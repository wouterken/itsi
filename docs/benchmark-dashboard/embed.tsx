import React from "react";
import ReactDOM from "react-dom/client";
import { BenchmarkDashboard } from "@/components/benchmark-dashboard";
import "@/styles/globals.css";

const root = document.getElementById("root");
if (root) {
  fetch("/results.json")
    .then((res) => res.json())
    .then((data) => {
      ReactDOM.createRoot(root).render(<BenchmarkDashboard data={data} />);
    });
}
