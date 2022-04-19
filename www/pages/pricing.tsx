import React from "react";
import Price from "../components/Price";

export function getStaticProps() {
  return {
    notFound: process.env.NODE_ENV === "production",
  };
}

export default function Pricing() {
  return (
    <>
      <Price />
    </>
  );
}
