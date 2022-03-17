import { withPageAuthRequired } from "@auth0/nextjs-auth0";
import React from "react";
import { useMount } from "react-use";
import ApiKeyModal, { useApiKeyModalState } from "../components/ApiKeyModal";
import Hero from "../components/Hero";

export default function Home() {
  const [open, setOpen] = useApiKeyModalState()

  useMount(() => {
    setOpen(true)
  })

  return (
    <>
      <Hero />
      <ApiKeyModal />
    </>
  );
}

export const getServerSideProps = withPageAuthRequired();
