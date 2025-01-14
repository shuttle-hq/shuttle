.PHONY: clean deep-clean

clean:
	rm .shuttle-*
	rm docker-compose.rendered.yml

deep-clean:
	find . -type d \( -name target -or -name .shuttle-executables -or -name node_modules \) | xargs rm -rf
