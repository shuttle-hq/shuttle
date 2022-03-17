import { withPageAuthRequired } from "@auth0/nextjs-auth0";
import React from "react";
import { useMount } from "react-use";
import Home from ".";
import { useApiKeyModalState } from "../components/ApiKeyModal";

export default function Login() {
  const [open, setOpen] = useApiKeyModalState();

  useMount(() => {
    setOpen(true);
  });

  return <Home />;
}

export const getServerSideProps = withPageAuthRequired();
