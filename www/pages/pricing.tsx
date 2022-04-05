import React from "react";
import ApiKeyModal from "../components/ApiKeyModal";
import Footer from "../components/Footer";
import Header from "../components/Header";
import Price from "../components/Price";

export function getStaticProps() {
  return {
    notFound: process.env.NODE_ENV === "production",
  };
}

export default function Pricing() {
  return (
    <>
      <Header />
      <Price />
      <ApiKeyModal />
      <Footer />
    </>
  );
}
