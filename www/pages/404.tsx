export default function NotFound() {
  return (
    <>
      <div className="m-auto w-10/12 max-w-2xl overflow-visible pt-32 pb-32 leading-none">
        <div className="m-auto w-full max-w-2xl">
          <div className="m-auto pb-4 text-6xl">
            <span className="m-auto font-bold text-brand-600">Oops!</span>
          </div>
          <div className="text-3xl">
            This page does not seem to exist, sorry.
          </div>
        </div>
      </div>
    </>
  );
}
