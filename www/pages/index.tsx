import React from "react";
import ApiKeyModal from "../components/ApiKeyModal";
import Cards from "../components/Cards";
import CodeSnippets from "../components/CodeSnippets";
import Header from "../components/Header";
import Hero from "../components/Hero";

export default function Home() {
  return (
    <>
      <Header />
      <Hero />
      <Cards />
      <CodeSnippets />
      <ApiKeyModal />
    </>
  );
}
