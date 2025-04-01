FROM cgr.dev/chainguard/jdk

WORKDIR /app

COPY ../java/src/IPv6Tester.java /app/IPv6Tester.java

ENTRYPOINT [ "java" ]

EXPOSE 8080

CMD [ "IPv6Tester.java", "server" ]
