import React from 'react'

import Hero from '../components/Hero'
import UseCases from '../components/UseCases'
import Features from '../components/Features'
import Examples from '../components/Examples'
import CallToAction from "../components/CallToAction"

export default function Home() {
    return <>
        <Hero/>
        <UseCases/>
        <Examples/>
        <Features/>
        <CallToAction/>
    </>
}
