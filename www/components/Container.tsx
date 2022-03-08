import { ReactNode, FunctionComponent } from 'react'

type Props = {
    children?: ReactNode,
    bgColor: string
}

const Container: FunctionComponent = ({children, bgColor}: Props) => {
    return (
        <div className={`w-full ${bgColor}`}>
            <div className="container w-8/12 mx-auto">
                {children}
            </div>
        </div>
    )
}

export default Container