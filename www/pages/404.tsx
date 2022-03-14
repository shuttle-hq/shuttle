const NotFound = () => {
  return (
    <>
      <div className="pt-32 pb-32 w-10/12 max-w-2xl m-auto leading-none overflow-visible">
        <div className="w-full max-w-2xl m-auto">
          <div className="text-6xl m-auto pb-4">
            <span className="text-brand-600 m-auto font-bold">
              Oops!
            </span>
          </div>
          <div className="text-3xl">
            This page does not seem to exist, sorry.
          </div>
        </div>
      </div>
    </>
  );
};

export default NotFound;
