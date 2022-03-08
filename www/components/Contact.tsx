import Section from './Section'

const Contact = () => {
    return <div className="relative w-full bg-gray-600">
        <div className="container w-10/12 sm:w-10/12 lg:w-10/12 xl:w-8/12 pt-16 sm:pt-32 pb-8 sm:pb-16 mx-auto">
            <div className="text-4xl font-bold font-Gilroy pb-10 max-w-lg m-auto">
                Drop us a line.
            </div>
            <MyForm/>
        </div>
    </div>
}

import {useForm} from '@formspree/react';

function MyForm() {
    const [state, handleSubmit] = useForm('maypbzgq');
    if (state.succeeded) {
        return <div>Thank you for your message! We'll get back to you shortly.</div>;
    }
    // @ts-ignore
    return (
        <form className="w-full max-w-lg m-auto" onSubmit={handleSubmit}>
            <div className="flex flex-wrap -mx-3 mb-6">
                <div className="w-full md:w-1/2 px-3 mb-6 md:mb-0">
                    <label className="block uppercase tracking-wide  text-xs font-bold mb-2"
                           htmlFor="Contact-Name">
                        Name
                    </label>
                    <input
                        className="appearance-none block w-full bg-gray-200 border border-gray-200 rounded py-3 px-4 mb-3 leading-tight focus:outline-none focus:bg-white text-gray-500"
                        id="Contact-Name" type="text" placeholder="James Bond" data-name="Contact-Name" name="Contact-Name"/>
                </div>
                <div className="w-full md:w-1/2 px-3">
                    <label className="block uppercase tracking-wide text-xs font-bold mb-2"
                           htmlFor="Contact-Email">
                        Email
                    </label>
                    <input
                        className="appearance-none block w-full bg-gray-200 border border-gray-200 rounded py-3 px-4 leading-tight focus:outline-none focus:bg-white focus:border-gray-500 text-gray-500"
                        id="Contact-Email" type="email" placeholder="james@bond.com" data-name="Contact-Email" name="Contact-Email"/>
                </div>
            </div>
            <div className="flex flex-wrap -mx-3 mb-6">
                <div className="w-full px-3">
                    <label className="block uppercase tracking-wide text-xs font-bold mb-2"
                           htmlFor="Contact-Subject">
                        Subject
                    </label>
                    <input
                        className="appearance-none block w-full bg-gray-200 border border-gray-200 rounded py-3 px-4 mb-3 leading-tight focus:outline-none focus:bg-white focus:border-gray-500 text-gray-500"
                        id="Contact-Subject" placeholder="Enter the subject" data-name="Contact-Subject" name="Contact-Subject"/>
                </div>
            </div>
            <div className="flex flex-wrap -mx-3 mb-6">
                <div className="w-full px-3">
                    <label className="block uppercase tracking-wide text-xs font-bold mb-2"
                           htmlFor="field">
                        Message
                    </label>
                    <textarea
                        className="text-area appearance-none block w-full bg-gray-200 border border-gray-200 rounded py-3 px-4 mb-3 leading-tight focus:outline-none focus:bg-white focus:border-gray-500 text-gray-500 h-32"
                        id="field" placeholder="How can we help you?" data-name="field" name="field"/>
                </div>
            </div>
            <div className="flex flex-wrap -mx-3 mb-6 w-full">
                <div className="w-full px-3 justify-center ">
                    <button
                        className="bg-white hover:bg-gray-100 text-gray-800 font-semibold py-2 px-4 border border-gray-400 rounded shadow"
                        type="submit" disabled={state.submitting}>Send Message
                    </button>
                </div>
            </div>

        </form>
    )
}

export default Contact;