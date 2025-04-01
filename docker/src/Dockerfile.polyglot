FROM registry.access.redhat.com/ubi9/ubi-minimal:9.3

# Update packages
RUN microdnf -y update && microdnf clean all

RUN microdnf -y install iproute iputils java-17-openjdk python3.12 && microdnf clean all

# Enable IPv6 in the container
RUN echo "net.ipv6.conf.all.disable_ipv6 = 0" >> /etc/sysctl.conf && \
    echo "net.ipv6.conf.default.disable_ipv6 = 0" >> /etc/sysctl.conf

WORKDIR /app

COPY ../java/src/IPv6Tester.java /app
COPY ../python/src/ipv6_tester.py /app

ENTRYPOINT [ "java" ]

EXPOSE 8080

CMD [ "IPv6Tester.java", "server" ]
