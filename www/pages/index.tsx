import React from "react";
import ApiKeyModal from "../components/ApiKeyModal";
import Header from "../components/Header";
import Hero from "../components/Hero";

export default function Home() {
  return (
    <>
      <Header />
      <Hero />
      <ApiKeyModal />
    </>
  );
}
