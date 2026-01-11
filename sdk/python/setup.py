from setuptools import setup, find_packages

setup(
    name="ruster-revm",
    version="0.1.0",
    description="Python SDK for Ruster REVM - Pre-Execution Risk Scoring (PERS) for Ethereum tokens",
    author="Ruster REVM Team",
    author_email="sdk@ruster-revm.io",
    url="https://github.com/nirvagold/ruster-revm",
    packages=find_packages(),
    install_requires=[
        "httpx>=0.24.0",
    ],
    extras_require={
        "async": ["httpx[http2]>=0.24.0"],
    },
    python_requires=">=3.8",
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Topic :: Software Development :: Libraries :: Python Modules",
        "Topic :: Security",
    ],
)
