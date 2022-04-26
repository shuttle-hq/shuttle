import { Fragment, useState, createContext, useContext } from "react";
import { Dialog, Transition } from "@headlessui/react";
import { XIcon } from "@heroicons/react/outline";
import { createStateContext } from "react-use";
import { useUser } from "@auth0/nextjs-auth0";
import Code from "./Code";
import mixpanel from "mixpanel-browser";
import { DISCORD_URL } from "../lib/constants";
import ExternalLink from "./ExternalLink";

export const [useApiKeyModalState, ApiKeyModalStateProvider] =
  createStateContext(false);

export default function ApiKeyModal() {
  const [open, setOpen] = useApiKeyModalState();
  const { user, error, isLoading } = useUser();

  const api_key = user?.api_key as string | undefined;

  return (
    <Transition.Root show={open} as={Fragment}>
      <Dialog
        as="div"
        className="fixed inset-0 z-40 overflow-y-auto dark:text-dark-200"
        onClose={setOpen}
      >
        <div className="flex min-h-screen items-end justify-center px-4 pt-4 pb-20 text-center sm:block sm:p-0">
          <Transition.Child
            as={Fragment}
            enter="ease-out duration-300"
            enterFrom="opacity-0"
            enterTo="opacity-100"
            leave="ease-in duration-200"
            leaveFrom="opacity-100"
            leaveTo="opacity-0"
          >
            <Dialog.Overlay className="fixed inset-0 bg-gray-500/25 transition-opacity dark:bg-gray-500/75" />
          </Transition.Child>

          {/* This element is to trick the browser into centering the modal contents. */}
          <span
            className="hidden sm:inline-block sm:h-screen sm:align-middle"
            aria-hidden="true"
          >
            &#8203;
          </span>
          <Transition.Child
            as={Fragment}
            enter="ease-out duration-300"
            enterFrom="opacity-0 translate-y-4 sm:translate-y-0 sm:scale-95"
            enterTo="opacity-100 translate-y-0 sm:scale-100"
            leave="ease-in duration-200"
            leaveFrom="opacity-100 translate-y-0 sm:scale-100"
            leaveTo="opacity-0 translate-y-4 sm:translate-y-0 sm:scale-95"
          >
            <div className="relative inline-block transform overflow-hidden rounded-lg bg-slate-100 text-left align-bottom shadow-xl transition-all dark:bg-dark-600 sm:my-8 sm:w-full sm:max-w-2xl sm:align-middle">
              <div className="absolute top-0 right-0 hidden pt-4 pr-4 sm:block">
                <button
                  type="button"
                  className="rounded border-dark-300 bg-slate-200 text-slate-800 hover:brightness-95 dark:border-dark-700 dark:bg-dark-600 dark:text-dark-200 dark:hover:brightness-125"
                  onClick={() => setOpen(false)}
                >
                  <span className="sr-only">Close</span>
                  <XIcon className="h-6 w-6" aria-hidden="true" />
                </button>
              </div>
              <div className="px-4 pt-5 pb-4 sm:p-6 sm:pb-4">
                {user && (
                  <>
                    {api_key && (
                      <div className="sm:flex sm:items-start">
                        <div className="mt-3 text-center sm:mt-0 sm:text-left">
                          <Dialog.Title
                            as="h3"
                            className="text-2xl font-medium leading-6 text-slate-800 dark:text-dark-200"
                          >
                            Api key
                          </Dialog.Title>
                          <div className="mt-2">
                            <p className="mb-2 text-xl text-slate-800 dark:text-dark-200">
                              copy/paste the API key below to the "cargo shuttle
                              login" dialog:
                            </p>
                            <Code id="api-key" code={api_key} />

                            <p className="mb-2 mt-2 text-xl text-slate-800 dark:text-dark-200">
                              alternatively, you can execute the command below:
                            </p>
                            <Code
                              id="cargo-shuttle-login-api-key"
                              code={`cargo shuttle login --api-key ${api_key}`}
                            />
                          </div>
                        </div>
                      </div>
                    )}
                    {!api_key && (
                      <div className="sm:flex sm:items-start">
                        <div className="mt-3 text-center sm:mt-0 sm:text-left">
                          <Dialog.Title
                            as="h3"
                            className="text-2xl font-medium leading-6 text-slate-800 dark:text-dark-200"
                          >
                            Api key not found!
                          </Dialog.Title>
                          <div className="mt-2">
                            <p className="text-xl text-slate-800 dark:text-dark-200">
                              {"This shouldn't happen. Please contact us on "}
                              <ExternalLink
                                className=":darkhover:brightness-125 underline hover:brightness-95"
                                href={DISCORD_URL}
                              >
                                Discord
                              </ExternalLink>
                              {" to resolve the issue"}
                            </p>
                          </div>
                        </div>
                      </div>
                    )}
                  </>
                )}
              </div>

              <div className="bg-slate-200/60 px-4 py-3 dark:bg-dark-500/40 sm:flex sm:flex-row-reverse sm:px-6">
                <button
                  type="button"
                  className="mt-3 inline-flex w-full justify-center rounded border border-slate-300 bg-slate-200 px-4 py-2 text-base font-medium text-slate-800 shadow-sm hover:brightness-95 dark:border-dark-700 dark:bg-dark-600 dark:text-dark-200 dark:hover:brightness-125 sm:mt-0 sm:ml-3 sm:w-auto sm:text-sm"
                  onClick={() => setOpen(false)}
                >
                  Close
                </button>
              </div>
            </div>
          </Transition.Child>
        </div>
      </Dialog>
    </Transition.Root>
  );
}
