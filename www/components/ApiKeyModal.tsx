import { Fragment, useState, createContext, useContext } from "react";
import { Dialog, Transition } from "@headlessui/react";
import { XIcon } from "@heroicons/react/outline";
import { createStateContext } from "react-use";
import { useUser } from "@auth0/nextjs-auth0";
import Code from "./Code";

export const [useApiKeyModalState, ApiKeyModalStateProvider] =
  createStateContext(false);

export default function ApiKeyModal() {
  const [open, setOpen] = useApiKeyModalState();
  const { user, error, isLoading } = useUser();

  const api_key = user.api_key as string

  return (
    <Transition.Root show={open} as={Fragment}>
      <Dialog
        as="div"
        className="fixed z-10 inset-0 overflow-y-auto text-dark-200"
        onClose={setOpen}
      >
        <div className="flex items-end justify-center min-h-screen pt-4 px-4 pb-20 text-center sm:block sm:p-0">
          <Transition.Child
            as={Fragment}
            enter="ease-out duration-300"
            enterFrom="opacity-0"
            enterTo="opacity-100"
            leave="ease-in duration-200"
            leaveFrom="opacity-100"
            leaveTo="opacity-0"
          >
            <Dialog.Overlay className="fixed inset-0 bg-gray-500 bg-opacity-75 transition-opacity" />
          </Transition.Child>

          {/* This element is to trick the browser into centering the modal contents. */}
          <span
            className="hidden sm:inline-block sm:align-middle sm:h-screen"
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
            <div className="relative inline-block align-bottom bg-dark-600 rounded-lg text-left overflow-hidden shadow-xl transform transition-all sm:my-8 sm:align-middle sm:max-w-2xl sm:w-full">
              <div className="hidden sm:block absolute top-0 right-0 pt-4 pr-4">
                <button
                  type="button"
                  className="border-dark-700 bg-dark-600  text-dark-200 hover:brightness-125 rounded-md "
                  onClick={() => setOpen(false)}
                >
                  <span className="sr-only">Close</span>
                  <XIcon className="h-6 w-6" aria-hidden="true" />
                </button>
              </div>
              <div className="px-4 pt-5 pb-4 sm:p-6 sm:pb-4">
                {user && (
                  <>
                    {user.api_key && (
                      <div className="sm:flex sm:items-start">
                        <div className="mt-3 text-center sm:mt-0 sm:text-left">
                          <Dialog.Title
                            as="h3"
                            className="text-2xl leading-6 font-medium text-dark-200"
                          >
                            Api key
                          </Dialog.Title>
                          <div className="mt-2">
                            <p className="text-xl text-dark-200 mb-2">
                              copy this api key to "cargo shuttle login" dialog
                            </p>
                            <Code code={api_key} />

                            <p className="text-xl text-dark-200 mb-2 mt-2">
                              alternativly you can execute this command
                            </p>
                            <Code code={`cargo shuttle login --api-key ${api_key}`}/>
                          </div>
                        </div>
                      </div>
                    )}
                    {!user.api_key && (
                      <div className="sm:flex sm:items-start">
                        <div className="mt-3 text-center sm:mt-0 sm:text-left">
                          <Dialog.Title
                            as="h3"
                            className="text-2xl leading-6 font-medium text-dark-200"
                          >
                            Api key not found!
                          </Dialog.Title>
                          <div className="mt-2">
                            <p className="text-xl text-dark-200">
                              Contact us on discord to resolve this issue
                            </p>
                          </div>
                        </div>
                      </div>
                    )}
                  </>
                )}
              </div>

              <div className="bg-dark-500/40 px-4 py-3 sm:px-6 sm:flex sm:flex-row-reverse">
                <button
                  type="button"
                  className="mt-3 w-full inline-flex justify-center rounded-md border border-dark-700 shadow-sm px-4 py-2 bg-dark-600 text-base font-medium text-dark-200 hover:brightness-125 sm:mt-0 sm:ml-3 sm:w-auto sm:text-sm"
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
