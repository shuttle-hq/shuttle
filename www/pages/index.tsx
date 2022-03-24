import React from "react";
import ApiKeyModal from "../components/ApiKeyModal";
import Cards from "../components/Cards";
import Cards2 from "../components/Cards2";
import CodeSnippets from "../components/CodeSnippets";
import Header from "../components/Header";
import Hero from "../components/Hero";

export default function Home() {
  return (
    <>
      <Header />
      <Hero />
      <Cards />
      <Cards2 />
      <CodeSnippets />
      <ApiKeyModal />
    </>
  );
}
